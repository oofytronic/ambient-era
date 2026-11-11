//! The mailbox file. Not a wire format.
//!
//! `mailboxes` sits in a directory the operator chose. Missing means empty.
//! A file that does not parse is refused, so a bad read cannot be saved back
//! over the real mail. The bytes inside a blob are not inspected.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::path::Path;

use super::{MAX_BLOB, RelayError, Slot};

const MAGIC: &[u8] = b"AMBX";
const VERSION: u32 = 1;

pub(super) fn load(dir: &Path) -> Result<BTreeMap<[u8; 32], Slot>, RelayError> {
    let path = dir.join("mailboxes");
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let bytes = fs::read(&path).map_err(|_| RelayError::Store)?;
    parse(&bytes).ok_or(RelayError::Store)
}

pub(super) fn save(dir: &Path, mailboxes: &BTreeMap<[u8; 32], Slot>) -> Result<(), RelayError> {
    fs::create_dir_all(dir).map_err(|_| RelayError::Store)?;
    tighten(dir, 0o700);
    let tmp = dir.join("mailboxes.tmp");
    let dest = dir.join("mailboxes");
    {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&tmp)
            .map_err(|_| RelayError::Store)?;
        write_store(&mut file, mailboxes)?;
        file.sync_all().map_err(|_| RelayError::Store)?;
    }
    tighten(&tmp, 0o600);
    fs::rename(&tmp, &dest).map_err(|_| RelayError::Store)?;
    Ok(())
}

fn write_store(file: &mut File, mailboxes: &BTreeMap<[u8; 32], Slot>) -> Result<(), RelayError> {
    file.write_all(MAGIC).map_err(|_| RelayError::Store)?;
    file.write_all(&VERSION.to_le_bytes())
        .map_err(|_| RelayError::Store)?;
    let count = u32::try_from(mailboxes.len()).map_err(|_| RelayError::Store)?;
    file.write_all(&count.to_le_bytes())
        .map_err(|_| RelayError::Store)?;
    for (id, slot) in mailboxes {
        file.write_all(id).map_err(|_| RelayError::Store)?;
        file.write_all(&slot.next.to_le_bytes())
            .map_err(|_| RelayError::Store)?;
        let items = u32::try_from(slot.items.len()).map_err(|_| RelayError::Store)?;
        file.write_all(&items.to_le_bytes())
            .map_err(|_| RelayError::Store)?;
        for (cursor, blob) in &slot.items {
            file.write_all(&cursor.to_le_bytes())
                .map_err(|_| RelayError::Store)?;
            let len = u32::try_from(blob.len()).map_err(|_| RelayError::Store)?;
            file.write_all(&len.to_le_bytes())
                .map_err(|_| RelayError::Store)?;
            file.write_all(blob).map_err(|_| RelayError::Store)?;
        }
    }
    Ok(())
}

fn parse(bytes: &[u8]) -> Option<BTreeMap<[u8; 32], Slot>> {
    let mut reader = Reader { bytes, at: 0 };
    if reader.take(MAGIC.len())? != MAGIC {
        return None;
    }
    if reader.u32()? != VERSION {
        return None;
    }
    let count = reader.u32()? as usize;
    if count > bytes.len() / 44 {
        return None;
    }
    let mut mailboxes = BTreeMap::new();
    for _ in 0..count {
        let id: [u8; 32] = reader.take(32)?.try_into().ok()?;
        let next = reader.u64()?;
        if next == 0 {
            return None;
        }
        let items = reader.u32()? as usize;
        if items > reader.remaining() / 12 {
            return None;
        }
        let mut stored = Vec::with_capacity(items);
        let mut previous = 0_u64;
        for _ in 0..items {
            let cursor = reader.u64()?;
            if cursor == 0 || cursor >= next || cursor <= previous {
                return None;
            }
            let len = reader.u32()? as usize;
            if len > MAX_BLOB || len > reader.remaining() {
                return None;
            }
            stored.push((cursor, reader.take(len)?.to_vec()));
            previous = cursor;
        }
        if mailboxes
            .insert(
                id,
                Slot {
                    next,
                    items: stored,
                },
            )
            .is_some()
        {
            return None;
        }
    }
    if reader.remaining() != 0 {
        return None;
    }
    Some(mailboxes)
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.at)
    }

    fn take(&mut self, len: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(len)?;
        let slice = self.bytes.get(self.at..end)?;
        self.at = end;
        Some(slice)
    }

    fn u32(&mut self) -> Option<u32> {
        let bytes = self.take(4)?;
        Some(u32::from_le_bytes(bytes.try_into().ok()?))
    }

    fn u64(&mut self) -> Option<u64> {
        let bytes = self.take(8)?;
        Some(u64::from_le_bytes(bytes.try_into().ok()?))
    }
}

fn tighten(path: &Path, mode: u32) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
    }
}
