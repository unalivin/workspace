use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use flate2::Compression;
use flate2::write::ZlibEncoder;

use crate::clock::Timestamp;

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(label: &str) -> Self {
        let number = COUNTER.fetch_add(1, Ordering::SeqCst);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        let base = fs::canonicalize(std::env::temp_dir()).unwrap_or_else(|_| std::env::temp_dir());
        let path = base.join(format!("secretsentry-test-{label}-{}-{number}-{stamp}", std::process::id()));
        fs::create_dir_all(&path).expect("create a temporary directory");
        TempDir { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub fn fixed_time() -> Timestamp {
    Timestamp::from_civil(2026, 3, 2, 9, 30, 0).expect("a valid fixed time")
}

pub fn sample_letters(length: usize) -> String {
    "q7Zp2LmX9vKc4RtB8nWd3FgH6jS1uYeOaIxNhTbVcMzQrPl5".chars().cycle().take(length).collect()
}

pub fn sample_cloud_key() -> String {
    ["AK", "IA", "Q4WZ7PLM2K6XTR3N"].concat()
}

pub fn sample_hosting_token() -> String {
    ["gh", "p_", &sample_letters(36)].concat()
}

pub fn sample_package_token() -> String {
    ["np", "m_", &sample_letters(36)].concat()
}

pub fn sha1(data: &[u8]) -> [u8; 20] {
    let mut state: [u32; 5] = [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476, 0xc3d2_e1f0];
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&(data.len() as u64 * 8).to_be_bytes());
    for block in message.as_chunks::<64>().0 {
        let mut words = [0u32; 80];
        for (word, bytes) in words.iter_mut().zip(block.as_chunks::<4>().0) {
            *word = u32::from_be_bytes(*bytes);
        }
        for index in 16..80 {
            words[index] = (words[index - 3] ^ words[index - 8] ^ words[index - 14] ^ words[index - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = state;
        for (index, word) in words.iter().enumerate() {
            let (mixed, constant) = match index / 20 {
                0 => ((b & c) | (!b & d), 0x5a82_7999u32),
                1 => (b ^ c ^ d, 0x6ed9_eba1),
                2 => ((b & c) | (b & d) | (c & d), 0x8f1b_bcdc),
                _ => (b ^ c ^ d, 0xca62_c1d6),
            };
            let next = a.rotate_left(5).wrapping_add(mixed).wrapping_add(e).wrapping_add(constant).wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = next;
        }
        for (total, part) in state.iter_mut().zip([a, b, c, d, e]) {
            *total = total.wrapping_add(part);
        }
    }
    let mut digest = [0u8; 20];
    for (target, part) in digest.as_chunks_mut::<4>().0.iter_mut().zip(state) {
        *target = part.to_be_bytes();
    }
    digest
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn unhex(text: &str) -> Vec<u8> {
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let digit = |byte: u8| char::from(byte).to_digit(16).expect("a hexadecimal digit");
            u8::try_from(digit(pair[0]) * 16 + digit(pair[1])).expect("one byte")
        })
        .collect()
}

fn deflate(data: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).expect("compress in memory");
    encoder.finish().expect("compress in memory")
}

fn with_header(kind: &str, body: &[u8]) -> Vec<u8> {
    let mut data = format!("{kind} {}\0", body.len()).into_bytes();
    data.extend_from_slice(body);
    data
}

pub fn git_object_id(kind: &str, body: &[u8]) -> String {
    hex(&sha1(&with_header(kind, body)))
}

fn size_bytes(mut size: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    while size >= 0x80 {
        bytes.push((size & 0x7f) as u8 | 0x80);
        size >>= 7;
    }
    bytes.push(size as u8);
    bytes
}

fn copy_instruction(offset: usize, size: usize) -> Vec<u8> {
    let mut instruction = vec![0x80u8];
    for bit in 0..4 {
        let byte = (offset >> (8 * bit)) as u8;
        if byte != 0 {
            instruction[0] |= 1 << bit;
            instruction.push(byte);
        }
    }
    for bit in 0..2 {
        let byte = (size >> (8 * bit)) as u8;
        if byte != 0 {
            instruction[0] |= 0x10 << bit;
            instruction.push(byte);
        }
    }
    instruction
}

pub fn make_delta(base: &[u8], target: &[u8]) -> Vec<u8> {
    let mut delta = size_bytes(base.len());
    delta.extend(size_bytes(target.len()));
    let shortest = base.len().min(target.len());
    let prefix = base.iter().zip(target).take_while(|(old, new)| old == new).count().min(0xffff);
    let suffix = base
        .iter()
        .rev()
        .zip(target.iter().rev())
        .take_while(|(old, new)| old == new)
        .count()
        .min(shortest - prefix)
        .min(0xffff);
    if prefix > 0 {
        delta.extend(copy_instruction(0, prefix));
    }
    for chunk in target[prefix..target.len() - suffix].chunks(127) {
        delta.push(chunk.len() as u8);
        delta.extend_from_slice(chunk);
    }
    if suffix > 0 {
        delta.extend(copy_instruction(base.len() - suffix, suffix));
    }
    delta
}

fn entry_header(code: u8, size: usize) -> Vec<u8> {
    let mut bytes = vec![(code << 4) | (size & 0x0f) as u8];
    let mut rest = size >> 4;
    while rest > 0 {
        let last = bytes.len() - 1;
        bytes[last] |= 0x80;
        bytes.push((rest & 0x7f) as u8);
        rest >>= 7;
    }
    bytes
}

fn kind_code(kind: &str) -> u8 {
    match kind {
        "commit" => 1,
        "tree" => 2,
        "blob" => 3,
        _ => 4,
    }
}

pub fn pack_entry_whole(kind: &str, body: &[u8]) -> Vec<u8> {
    let mut entry = entry_header(kind_code(kind), body.len());
    entry.extend(deflate(body));
    entry
}

pub fn pack_entry_offset_delta(distance: u64, delta: &[u8]) -> Vec<u8> {
    let mut entry = entry_header(6, delta.len());
    let mut bytes = vec![(distance & 0x7f) as u8];
    let mut rest = distance >> 7;
    while rest > 0 {
        rest -= 1;
        bytes.push(0x80 | (rest & 0x7f) as u8);
        rest >>= 7;
    }
    bytes.reverse();
    entry.extend(bytes);
    entry.extend(deflate(delta));
    entry
}

pub fn pack_entry_reference_delta(base_id: &str, delta: &[u8]) -> Vec<u8> {
    let mut entry = entry_header(7, delta.len());
    entry.extend(unhex(base_id));
    entry.extend(deflate(delta));
    entry
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeltaStyle {
    None,
    Offset,
    Reference,
}

pub struct GitFixture {
    git_dir: PathBuf,
}

impl GitFixture {
    pub fn create(root: &Path) -> GitFixture {
        let git_dir = root.join(".git");
        fs::create_dir_all(git_dir.join("objects").join("pack")).expect("create the object directory");
        fs::create_dir_all(git_dir.join("refs").join("heads")).expect("create the reference directory");
        fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").expect("write HEAD");
        fs::write(git_dir.join("config"), "[core]\n\trepositoryformatversion = 0\n\tbare = false\n")
            .expect("write the configuration");
        GitFixture { git_dir }
    }

    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    pub fn object_path(&self, id: &str) -> PathBuf {
        self.git_dir.join("objects").join(&id[..2]).join(&id[2..])
    }

    pub fn object(&self, kind: &str, body: &[u8]) -> String {
        let id = git_object_id(kind, body);
        let path = self.object_path(&id);
        fs::create_dir_all(path.parent().expect("an object directory")).expect("create the object directory");
        fs::write(path, deflate(&with_header(kind, body))).expect("write an object");
        id
    }

    pub fn blob(&self, content: &str) -> String {
        self.object("blob", content.as_bytes())
    }

    pub fn tree_body(entries: &[(&str, &str, &str)]) -> Vec<u8> {
        let mut sorted = entries.to_vec();
        sorted.sort_by(|first, second| first.1.cmp(second.1));
        let mut body = Vec::new();
        for (mode, name, id) in sorted {
            body.extend_from_slice(format!("{mode} {name}\0").as_bytes());
            body.extend(unhex(id));
        }
        body
    }

    pub fn tree(&self, entries: &[(&str, &str, &str)]) -> String {
        self.object("tree", &GitFixture::tree_body(entries))
    }

    pub fn commit_body(tree: &str, parents: &[&str], time: i64, subject: &str) -> Vec<u8> {
        let mut lines = vec![format!("tree {tree}")];
        lines.extend(parents.iter().map(|parent| format!("parent {parent}")));
        lines.push(format!("author Sample Author <author> {time} +0000"));
        lines.push(format!("committer Sample Committer <committer> {time} +0000"));
        lines.push(String::new());
        lines.push(subject.to_string());
        let mut text = lines.join("\n");
        text.push('\n');
        text.into_bytes()
    }

    pub fn commit(&self, tree: &str, parents: &[&str], time: i64, subject: &str) -> String {
        self.object("commit", &GitFixture::commit_body(tree, parents, time, subject))
    }

    pub fn tag(&self, target: &str, name: &str) -> String {
        let body = format!(
            "object {target}\ntype commit\ntag {name}\ntagger Sample Tagger <tagger> 1700000000 +0000\n\n{name}\n"
        );
        self.object("tag", body.as_bytes())
    }

    pub fn set_reference(&self, name: &str, value: &str) {
        let path = self.git_dir.join(name);
        fs::create_dir_all(path.parent().expect("a reference directory")).expect("create the reference directory");
        fs::write(path, format!("{value}\n")).expect("write a reference");
    }

    pub fn write_pack(&self, name: &str, entries: &[(String, Vec<u8>)]) {
        let mut pack = b"PACK".to_vec();
        pack.extend(2u32.to_be_bytes());
        pack.extend((entries.len() as u32).to_be_bytes());
        let mut placed: Vec<(Vec<u8>, u32)> = Vec::new();
        for (id, entry) in entries {
            placed.push((unhex(id), pack.len() as u32));
            pack.extend_from_slice(entry);
        }
        pack.extend([0u8; 20]);
        placed.sort();
        let mut index = vec![0xff, b't', b'O', b'c'];
        index.extend(2u32.to_be_bytes());
        for slot in 0..=255u8 {
            let total = placed.iter().filter(|(id, _)| id[0] <= slot).count() as u32;
            index.extend(total.to_be_bytes());
        }
        for (id, _) in &placed {
            index.extend_from_slice(id);
        }
        index.extend(std::iter::repeat_n(0u8, placed.len() * 4));
        for (_, offset) in &placed {
            index.extend(offset.to_be_bytes());
        }
        index.extend([0u8; 40]);
        let directory = self.git_dir.join("objects").join("pack");
        fs::write(directory.join(format!("{name}.pack")), pack).expect("write a pack");
        fs::write(directory.join(format!("{name}.idx")), index).expect("write a pack index");
    }

    pub fn pack(&self, name: &str, objects: &[(&str, Vec<u8>)], style: DeltaStyle) -> Vec<String> {
        let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
        let mut offsets: Vec<u64> = Vec::new();
        let mut previous: BTreeMap<&str, usize> = BTreeMap::new();
        let mut offset = 12u64;
        for (number, (kind, body)) in objects.iter().enumerate() {
            let base = previous.get(kind).copied().filter(|_| style != DeltaStyle::None);
            let entry = match base {
                None => pack_entry_whole(kind, body),
                Some(base) => {
                    let delta = make_delta(&objects[base].1, body);
                    if style == DeltaStyle::Offset {
                        pack_entry_offset_delta(offset - offsets[base], &delta)
                    } else {
                        pack_entry_reference_delta(&entries[base].0, &delta)
                    }
                }
            };
            previous.insert(*kind, number);
            offsets.push(offset);
            offset += entry.len() as u64;
            entries.push((git_object_id(kind, body), entry));
        }
        self.write_pack(name, &entries);
        entries.into_iter().map(|(id, _)| id).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temporary_directories_are_unique_and_removed() {
        let first = TempDir::new("kit");
        let second = TempDir::new("kit");
        assert_ne!(first.path(), second.path());
        let kept = first.path().to_path_buf();
        fs::write(kept.join("file"), "x").unwrap();
        drop(first);
        assert!(!kept.exists());
        assert!(second.path().is_dir());
        assert_eq!(fixed_time().to_rfc3339(), "2026-03-02T09:30:00Z");
        assert_eq!(sample_letters(50).len(), 50);
        assert_eq!(hex(&sha1(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(hex(&sha1(&[b'x'; 200])), "94218caae9904e93a3d7bf578bf4791926fc5e82");
        assert_eq!(git_object_id("blob", b"hello\n"), "ce013625030ba8dba906f756967f9e9ca394464a");
        assert_eq!(unhex("00ff10"), vec![0, 255, 16]);
        assert_eq!(
            (sample_cloud_key().len(), sample_hosting_token().len(), sample_package_token().len()),
            (20, 40, 40)
        );
    }
}
