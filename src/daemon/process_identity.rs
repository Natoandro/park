use std::fs;
use std::io;

use crate::process::ProcessRecord;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ProcessIdentity {
    pub(super) start_time: u64,
    pub(super) process_group_id: u32,
    pub(super) session_id: u32,
}

#[cfg(target_os = "linux")]
pub(super) fn read(pid: u32) -> io::Result<ProcessIdentity> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat"))?;
    let (_, fields) = stat.rsplit_once(") ").ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "malformed Linux process stat")
    })?;
    let fields = fields.split_whitespace().collect::<Vec<_>>();
    let process_group_id = parse_id(&fields, 2)?;
    let session_id = parse_id(&fields, 3)?;
    let start_time = fields
        .get(19)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing process start time"))?
        .parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid process start time"))?;
    Ok(ProcessIdentity {
        start_time,
        process_group_id,
        session_id,
    })
}

#[cfg(target_os = "linux")]
fn parse_id(fields: &[&str], index: usize) -> io::Result<u32> {
    fields
        .get(index)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing process identifier"))?
        .parse::<i32>()
        .ok()
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid process identifier"))
}

#[cfg(not(target_os = "linux"))]
pub(super) fn read(_pid: u32) -> io::Result<ProcessIdentity> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "process identity verification requires Linux",
    ))
}

pub(super) fn matches_record(record: &ProcessRecord) -> bool {
    let (Some(pid), Some(group_id), Some(start_time)) = (
        record.pid(),
        record.process_group_id(),
        record.process_start_time(),
    ) else {
        return false;
    };
    let Ok(identity) = read(pid) else {
        return false;
    };
    identity.start_time == start_time
        && identity.process_group_id == group_id
        && identity.session_id == group_id
}

pub(super) fn owns_group(record: &ProcessRecord) -> bool {
    let Some(group_id) = record.process_group_id() else {
        return false;
    };
    if matches_record(record) {
        return true;
    }
    #[cfg(target_os = "linux")]
    {
        fs::read_dir("/proc").is_ok_and(|entries| {
            entries.flatten().any(|entry| {
                let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
                    return false;
                };
                read(pid).is_ok_and(|identity| {
                    identity.process_group_id == group_id && identity.session_id == group_id
                })
            })
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}
