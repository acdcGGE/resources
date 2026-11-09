use std::collections::HashMap;

use lazy_regex::{Lazy, Regex, lazy_regex};
use log::trace;

use crate::util::{extract_from_regex, read_parsed};

static RE_PSS: Lazy<Regex> = lazy_regex!(r"Pss:\s*([0-9]+)\s*kB");

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MappingKey {
    dev: u32, // packed major<<16 | minor
    inode: u64,
}

/// key -> resident bytes in this process
type FileMappings = HashMap<MappingKey, u64>;
type SharerCounts = HashMap<MappingKey, usize>;

#[derive(Debug, Default, Clone)]
pub struct PidMappings {
    file: FileMappings,
    shmem: FileMappings,
}

#[must_use]
pub fn read_exact_pss(pid: libc::pid_t) -> Option<usize> {
    read_parsed::<String>(format!("/proc/{pid}/smaps_rollup"))
        .ok()
        .map(|smaps_rollup| {
            extract_from_regex::<usize, _>(&RE_PSS, smaps_rollup)
                .unwrap_or_default()
                .saturating_mul(1024)
        })
}

fn parse_dev(s: &str) -> Option<u32> {
    let (maj, min) = s.split_once(':')?;
    let maj = u32::from_str_radix(maj, 16).ok()?;
    let min = u32::from_str_radix(min, 16).ok()?;
    Some((maj << 16) | (min & 0xFFFF))
}

fn is_shmem_backed(pathname: &str) -> bool {
    pathname.is_empty()
        || pathname.starts_with("/dev/shm/")
        || pathname.starts_with("/memfd:")
        || pathname.starts_with("/SYSV")
}

fn pid_mappings(pid: libc::pid_t) -> PidMappings {
    let mut file = HashMap::default();
    let mut shmem = HashMap::default();

    let Ok(content) = read_parsed::<String>(format!("/proc/{pid}/maps")) else {
        return PidMappings { file, shmem };
    };

    for line in content.lines() {
        let mut fields = line.split_whitespace();

        let Some(addr) = fields.next() else { continue };
        let _perms = fields.next();
        let _offset = fields.next();
        let Some(dev) = fields.next() else { continue };
        let Some(inode_str) = fields.next() else {
            continue;
        };

        let Ok(inode) = inode_str.parse::<u64>() else {
            continue;
        };
        if inode == 0 {
            continue;
        }

        let Some((start_str, end_str)) = addr.split_once('-') else {
            continue;
        };
        let (Ok(start), Ok(end)) = (
            u64::from_str_radix(start_str, 16),
            u64::from_str_radix(end_str, 16),
        ) else {
            continue;
        };

        let Some(packed_dev) = parse_dev(dev) else {
            continue;
        };

        // whatever's left (if anything) is the pathname
        let pathname = fields.next().unwrap_or("");

        let key = MappingKey {
            dev: packed_dev,
            inode,
        };
        let size = end.saturating_sub(start);

        let target = if is_shmem_backed(&pathname) {
            &mut shmem
        } else {
            &mut file
        };
        *target.entry(key).or_insert(0u64) += size;
    }

    trace!(
        "Found {} file mappings and {} shmem mappings for {pid}",
        file.len(),
        shmem.len()
    );
    PidMappings { file, shmem }
}

pub fn collect_file_mappings(
    pids: &[libc::pid_t],
) -> (HashMap<libc::pid_t, PidMappings>, SharerCounts) {
    trace!(
        "Collecting file/shmem mappings for {} processes",
        pids.len()
    );
    let mut all_mappings: HashMap<libc::pid_t, PidMappings> = HashMap::default();
    let mut sharer_counts: SharerCounts = HashMap::default();

    for &pid in pids {
        let mappings = pid_mappings(pid);
        if mappings.file.is_empty() && mappings.shmem.is_empty() {
            continue;
        }

        for key in mappings.file.keys().chain(mappings.shmem.keys()) {
            *sharer_counts.entry(key.clone()).or_insert(0) += 1;
        }

        all_mappings.insert(pid, mappings);
    }

    (all_mappings, sharer_counts)
}

fn weighted_rss(
    pid: libc::pid_t,
    label: &str,
    raw: usize,
    mappings: Option<&FileMappings>,
    sharer_counts: &SharerCounts,
) -> usize {
    let Some(mappings) = mappings.filter(|_| raw > 0) else {
        // nothing to weight against => report it unscaled rather than dropping it
        trace!("No {label} mappings (or rss == 0) for {pid}, using raw {label} RSS");
        return raw;
    };

    let total_vsz = mappings.values().sum::<u64>() as f64;
    if total_vsz == 0.0 {
        trace!("Total {label} VSZ is 0 for {pid}, using raw {label} RSS");
        return raw;
    }

    let mut weighted_share = 0.0f64;
    for (key, size) in mappings {
        let n_sharers = *sharer_counts.get(key).unwrap_or(&1) as f64;
        let weight = *size as f64 / total_vsz;
        weighted_share += weight / n_sharers;
    }

    trace!("Weighted sharing factor for {label} of {pid} determined to be {weighted_share:.3}");

    (raw as f64 * weighted_share.clamp(0.0, 1.0)).round() as usize
}

#[must_use]
pub fn approx_pss(
    pid: libc::pid_t,
    rss_anon: usize,
    rss_file: usize,
    rss_shmem: usize,
    mappings: Option<&PidMappings>,
    sharer_counts: &SharerCounts,
) -> usize {
    let weighted_file = weighted_rss(
        pid,
        "file",
        rss_file,
        mappings.map(|m| &m.file),
        sharer_counts,
    );
    let weighted_shmem = weighted_rss(
        pid,
        "shmem",
        rss_shmem,
        mappings.map(|m| &m.shmem),
        sharer_counts,
    );

    let approx_pss = rss_anon + weighted_file + weighted_shmem;
    trace!("Approx PSS of {pid} determined to be {approx_pss}");

    approx_pss
}
