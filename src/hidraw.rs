//! The MK3's HID interface through hidraw: find, open, read, write, report sizes.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use crate::protocol::{PID, VID};

pub struct Hidraw {
    file: File,
    pub path: PathBuf,
    /// Output report ID → length in bytes, ID included (from the report descriptor).
    pub output_len: HashMap<u8, usize>,
    /// Input report ID → length in bytes, ID included.
    pub input_len: HashMap<u8, usize>,
}

fn hid_id() -> String {
    format!("HID_ID=0003:0000{VID:04X}:0000{PID:04X}")
}

/// The hidraw nodes (`hidrawN`) whose sysfs uevent under `class` matches the MK3.
fn find_in(class: &Path) -> io::Result<Vec<PathBuf>> {
    let dir = match fs::read_dir(class) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(vec![]),
        r => r?,
    };
    let want = hid_id();
    let mut found: Vec<_> = dir
        .filter_map(|e| e.ok())
        .filter(|e| {
            fs::read_to_string(e.path().join("device/uevent"))
                .is_ok_and(|u| u.lines().any(|l| l.eq_ignore_ascii_case(&want)))
        })
        .map(|e| e.path())
        .collect();
    found.sort();
    Ok(found)
}

/// Minimal HID report descriptor parser: per (kind, report ID), the total size in bytes,
/// report ID included. kind: 0x80 Input, 0x90 Output, 0xB0 Feature.
pub fn report_sizes(desc: &[u8]) -> HashMap<(u8, u8), usize> {
    let mut bits: HashMap<(u8, u8), u32> = HashMap::new();
    let (mut id, mut size, mut count) = (0u8, 0u32, 0u32);
    let mut stack = Vec::new();
    let mut i = 0;
    while i < desc.len() {
        let prefix = desc[i];
        if prefix == 0xFE {
            // long item
            let len = *desc.get(i + 1).unwrap_or(&0) as usize;
            i += 3 + len;
            continue;
        }
        let len = [0, 1, 2, 4][(prefix & 0x03) as usize];
        let mut data = 0u32;
        for k in 0..len {
            data |= (*desc.get(i + 1 + k).unwrap_or(&0) as u32) << (8 * k);
        }
        match prefix & 0xFC {
            0x84 => id = data as u8,
            0x74 => size = data,
            0x94 => count = data,
            0xA4 => stack.push((id, size, count)),
            0xB4 => {
                if let Some(s) = stack.pop() {
                    (id, size, count) = s;
                }
            }
            k @ (0x80 | 0x90 | 0xB0) => *bits.entry((k, id)).or_default() += size * count,
            _ => {}
        }
        i += 1 + len;
    }
    bits.into_iter()
        .map(|(k, b)| (k, b.div_ceil(8) as usize + usize::from(k.1 != 0)))
        .collect()
}

impl Hidraw {
    pub fn open() -> io::Result<Hidraw> {
        let nodes = find_in(Path::new("/sys/class/hidraw"))?;
        let Some(node) = nodes.first() else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Maschine MK3 ({VID:04x}:{PID:04x}) not found"),
            ));
        };
        let sizes = fs::read(node.join("device/report_descriptor")).map(|d| report_sizes(&d)).unwrap_or_default();
        let pick = |kind| sizes.iter().filter(|((k, _), _)| *k == kind).map(|((_, id), l)| (*id, *l)).collect();
        let path = Path::new("/dev").join(node.file_name().unwrap());
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(&path)
            .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))?;
        Ok(Hidraw { file, path, output_len: pick(0x90), input_len: pick(0x80) })
    }

    /// A second handle on the same device, for writing from another thread.
    pub fn try_clone(&self) -> io::Result<Hidraw> {
        Ok(Hidraw {
            file: self.file.try_clone()?,
            path: self.path.clone(),
            output_len: self.output_len.clone(),
            input_len: self.input_len.clone(),
        })
    }

    /// One input report, ID byte first, or `None` when nothing is pending.
    pub fn read<'a>(&mut self, buf: &'a mut [u8]) -> io::Result<Option<&'a [u8]>> {
        match self.file.read(buf) {
            Ok(0) => Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => Ok(Some(&buf[..n])),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Writes output report `id` with `payload`, padded (or cut) to the descriptor's length.
    pub fn write_report(&mut self, id: u8, payload: &[u8], default_len: usize) -> io::Result<()> {
        let len = self.output_len.get(&id).copied().unwrap_or(default_len);
        let mut r = vec![0u8; len.max(1)];
        r[0] = id;
        let n = payload.len().min(len.saturating_sub(1));
        r[1..1 + n].copy_from_slice(&payload[..n]);
        let w = self.file.write(&r)?;
        if w != r.len() {
            return Err(io::Error::new(io::ErrorKind::WriteZero, "short hidraw write"));
        }
        Ok(())
    }

    /// GET_REPORT(input, `id`): the current state, ID byte first. `None` if the device refuses.
    pub fn get_input(&self, id: u8, default_len: usize) -> Option<Vec<u8>> {
        let len = self.input_len.get(&id).copied().unwrap_or(default_len);
        let mut buf = vec![0u8; len];
        buf[0] = id;
        // HIDIOCGINPUT(len) = _IOC(_IOC_READ | _IOC_WRITE, 'H', 0x0A, len)
        let req = (3 << 30 | (len as libc::c_ulong) << 16 | (b'H' as libc::c_ulong) << 8 | 0x0A) as _;
        // SAFETY: the ioctl writes at most `len` bytes, the size encoded in the request.
        let r = unsafe { libc::ioctl(self.file.as_raw_fd(), req, buf.as_mut_ptr()) };
        (r > 0).then(|| {
            buf.truncate(r as usize);
            buf
        })
    }

    pub fn fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find() {
        let dir = std::env::temp_dir().join(format!("mk3-test-{}", std::process::id()));
        for (node, id) in [("hidraw0", "0003:0000046D:0000C52B"), ("hidraw3", "0003:000017CC:00001600")] {
            fs::create_dir_all(dir.join(node).join("device")).unwrap();
            fs::write(dir.join(node).join("device/uevent"), format!("HID_ID={id}\n")).unwrap();
        }
        assert_eq!(find_in(&dir).unwrap(), vec![dir.join("hidraw3")]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn descriptor_sizes() {
        // Report ID 1: 8 bits x 41 input; report ID 0x80: 8 bits x 62 output
        let d = [
            0x85, 0x01, 0x75, 0x08, 0x95, 0x29, 0x81, 0x02, //
            0x85, 0x80, 0x95, 0x3e, 0x91, 0x02,
        ];
        let s = report_sizes(&d);
        assert_eq!(s[&(0x80, 1)], 42);
        assert_eq!(s[&(0x90, 0x80)], 63);
    }
}
