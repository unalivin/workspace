use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use flate2::read::ZlibDecoder;

use crate::error::{AppError, Result};
use crate::tree::BINARY_PROBE_BYTES;

pub const ID_LENGTH: usize = 20;
pub const MAX_OBJECT_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_DELTA_DEPTH: usize = 512;
pub const MAX_INDEX_BYTES: u64 = 1024 * 1024 * 1024;
pub const MAX_PACKS: usize = 4096;
pub const MAX_ALTERNATES: usize = 16;
pub const MAX_REFERENCES: usize = 100_000;
pub const MAX_REFERENCE_DEPTH: usize = 16;
pub const MAX_TREE_DEPTH: usize = 128;
pub const MAX_PARENTS: usize = 256;
pub const MAX_VISITED: usize = 5_000_000;
pub const DEFAULT_MAX_COMMITS: u64 = 200_000;

const CACHE_BYTES: usize = 64 * 1024 * 1024;
const CACHE_ITEM_BYTES: usize = 1024 * 1024;
const SMALL_FILE_BYTES: u64 = 64 * 1024;
const PACKED_REFERENCES_BYTES: u64 = 64 * 1024 * 1024;
const PACK_HEADER_BYTES: u64 = 12;
const INDEX_HEADER_BYTES: usize = 8 + 256 * 4;
const MODE_DIRECTORY: u32 = 0o040000;
const FILE_MODES: [u32; 3] = [0o100644, 0o100755, 0o100664];

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectId([u8; ID_LENGTH]);

impl ObjectId {
    pub fn from_bytes(bytes: &[u8]) -> Option<ObjectId> {
        <[u8; ID_LENGTH]>::try_from(bytes).ok().map(ObjectId)
    }

    pub fn from_hex(text: &str) -> Option<ObjectId> {
        let digits = text.as_bytes();
        if digits.len() != ID_LENGTH * 2 {
            return None;
        }
        let mut id = [0u8; ID_LENGTH];
        for (index, pair) in digits.as_chunks::<2>().0.iter().enumerate() {
            let high = char::from(pair[0]).to_digit(16)?;
            let low = char::from(pair[1]).to_digit(16)?;
            id[index] = u8::try_from(high * 16 + low).ok()?;
        }
        Some(ObjectId(id))
    }

    pub fn to_hex(self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    pub fn as_bytes(&self) -> &[u8; ID_LENGTH] {
        &self.0
    }
}

impl fmt::Debug for ObjectId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectKind {
    Commit,
    Tree,
    Blob,
    Tag,
}

impl ObjectKind {
    pub fn parse(text: &str) -> Option<ObjectKind> {
        match text {
            "commit" => Some(ObjectKind::Commit),
            "tree" => Some(ObjectKind::Tree),
            "blob" => Some(ObjectKind::Blob),
            "tag" => Some(ObjectKind::Tag),
            _ => None,
        }
    }

    fn from_code(code: u8) -> Option<ObjectKind> {
        match code {
            1 => Some(ObjectKind::Commit),
            2 => Some(ObjectKind::Tree),
            3 => Some(ObjectKind::Blob),
            4 => Some(ObjectKind::Tag),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fetched {
    Found(ObjectKind, Vec<u8>),
    TooLarge,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub tree: ObjectId,
    pub parents: Vec<ObjectId>,
    pub author: String,
    pub author_time: i64,
    pub commit_time: i64,
    pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    pub mode: u32,
    pub name: String,
    pub id: ObjectId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub name: String,
    pub id: ObjectId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInfo {
    pub id: ObjectId,
    pub author: String,
    pub time: i64,
    pub subject: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryOptions {
    pub max_blob_bytes: u64,
    pub max_commits: u64,
}

impl Default for HistoryOptions {
    fn default() -> Self {
        HistoryOptions { max_blob_bytes: crate::tree::DEFAULT_MAX_FILE_BYTES, max_commits: DEFAULT_MAX_COMMITS }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HistoryStats {
    pub references: u64,
    pub commits_scanned: u64,
    pub files_scanned: u64,
    pub bytes_scanned: u64,
    pub binary: u64,
    pub too_large: u64,
    pub too_deep: u64,
    pub unreadable: u64,
    pub missing: u64,
    pub skipped_packs: u64,
    pub cancelled: bool,
    pub limit_reached: bool,
}

impl HistoryStats {
    pub fn skipped(&self) -> u64 {
        self.binary + self.too_large + self.too_deep + self.unreadable + self.missing
    }

    pub fn complete(&self) -> bool {
        !self.cancelled && !self.limit_reached
    }
}

enum Fault {
    TooLarge,
    Broken(String),
}

impl From<AppError> for Fault {
    fn from(error: AppError) -> Self {
        Fault::Broken(error.to_string())
    }
}

type Faulty<T> = std::result::Result<T, Fault>;

fn damaged(what: &str) -> AppError {
    AppError::Io(format!("the Git data is damaged: {what}"))
}

fn broken(what: &str) -> Fault {
    Fault::Broken(what.to_string())
}

pub fn split_loose(data: &[u8]) -> Result<(ObjectKind, &[u8])> {
    let end =
        data.iter().take(32).position(|byte| *byte == 0).ok_or_else(|| damaged("a loose object has no header"))?;
    let header =
        std::str::from_utf8(&data[..end]).map_err(|_| damaged("the header of a loose object is not readable"))?;
    let (kind, size) = header.split_once(' ').ok_or_else(|| damaged("the header of a loose object has no size"))?;
    let kind = ObjectKind::parse(kind).ok_or_else(|| damaged("a loose object has an unknown type"))?;
    let size: u64 = size.parse().map_err(|_| damaged("the size of a loose object is not a number"))?;
    let body = &data[end + 1..];
    if body.len() as u64 != size {
        return Err(damaged("a loose object has another size than its header says"));
    }
    Ok((kind, body))
}

fn person(text: &str) -> (String, i64) {
    let name: String =
        text.split('<').next().unwrap_or_default().trim().chars().filter(|letter| !letter.is_control()).collect();
    let time = text
        .rsplit('>')
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .next()
        .and_then(|seconds| seconds.parse::<i64>().ok())
        .unwrap_or(0);
    (name, time)
}

pub fn parse_commit(body: &[u8]) -> Result<Commit> {
    let text = String::from_utf8_lossy(body);
    let (headers, message) = text.split_once("\n\n").unwrap_or((text.as_ref(), ""));
    let mut tree = None;
    let mut parents = Vec::new();
    let mut author = (String::new(), 0);
    let mut commit_time = 0;
    for line in headers.lines() {
        if let Some(rest) = line.strip_prefix("tree ") {
            tree = Some(ObjectId::from_hex(rest.trim()).ok_or_else(|| damaged("a commit names an unreadable tree"))?);
        } else if let Some(rest) = line.strip_prefix("parent ") {
            if parents.len() >= MAX_PARENTS {
                return Err(damaged("a commit has too many parents"));
            }
            parents.push(ObjectId::from_hex(rest.trim()).ok_or_else(|| damaged("a commit names an unreadable parent"))?);
        } else if let Some(rest) = line.strip_prefix("author ") {
            author = person(rest);
        } else if let Some(rest) = line.strip_prefix("committer ") {
            commit_time = person(rest).1;
        }
    }
    let subject: String = message
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .chars()
        .filter(|letter| !letter.is_control())
        .collect();
    Ok(Commit {
        tree: tree.ok_or_else(|| damaged("a commit has no tree"))?,
        parents,
        author: author.0,
        author_time: author.1,
        commit_time,
        subject,
    })
}

pub fn parse_tag(body: &[u8]) -> Result<ObjectId> {
    let text = String::from_utf8_lossy(body);
    let headers = text.split_once("\n\n").map_or(text.as_ref(), |(headers, _)| headers);
    headers
        .lines()
        .find_map(|line| line.strip_prefix("object "))
        .and_then(|rest| ObjectId::from_hex(rest.trim()))
        .ok_or_else(|| damaged("a tag does not name its object"))
}

pub fn parse_tree(body: &[u8]) -> Result<Vec<TreeEntry>> {
    let mut entries = Vec::new();
    let mut rest = body;
    while !rest.is_empty() {
        let space = rest
            .iter()
            .take(8)
            .position(|byte| *byte == b' ')
            .ok_or_else(|| damaged("a tree entry has no mode"))?;
        let mode = std::str::from_utf8(&rest[..space])
            .ok()
            .filter(|digits| !digits.is_empty() && digits.bytes().all(|digit| digit.is_ascii_digit()))
            .and_then(|digits| u32::from_str_radix(digits, 8).ok())
            .ok_or_else(|| damaged("a tree entry has an unreadable mode"))?;
        rest = &rest[space + 1..];
        let end = rest.iter().position(|byte| *byte == 0).ok_or_else(|| damaged("a tree entry has no name"))?;
        let name = String::from_utf8_lossy(&rest[..end]).into_owned();
        rest = &rest[end + 1..];
        let id = rest
            .get(..ID_LENGTH)
            .and_then(ObjectId::from_bytes)
            .ok_or_else(|| damaged("a tree entry ends inside its object name"))?;
        rest = &rest[ID_LENGTH..];
        if name.is_empty() || name == "." || name == ".." || name.contains('/') {
            return Err(damaged("a tree entry has an unusable name"));
        }
        entries.push(TreeEntry { mode, name, id });
    }
    Ok(entries)
}

fn delta_size(delta: &[u8], at: &mut usize) -> Result<u64> {
    let mut value = 0u64;
    let mut shift = 0u32;
    loop {
        let byte = *delta.get(*at).ok_or_else(|| damaged("a delta ends inside its header"))?;
        *at += 1;
        if shift > 56 {
            return Err(damaged("a delta has an unreadable size"));
        }
        value |= u64::from(byte & 0x7f) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
}

pub fn delta_result_size(delta: &[u8]) -> Result<u64> {
    let mut at = 0;
    delta_size(delta, &mut at)?;
    delta_size(delta, &mut at)
}

pub fn apply_delta(base: &[u8], delta: &[u8], limit: u64) -> Result<Vec<u8>> {
    let mut at = 0;
    let base_size = delta_size(delta, &mut at)?;
    let result_size = delta_size(delta, &mut at)?;
    if base_size != base.len() as u64 {
        return Err(damaged("a delta was made for another base"));
    }
    if result_size > limit {
        return Err(damaged("a delta produces an object above the size limit"));
    }
    let wanted = usize::try_from(result_size).map_err(|_| damaged("a delta has an unreadable size"))?;
    let mut out = Vec::with_capacity(wanted);
    while at < delta.len() {
        let operation = delta[at];
        at += 1;
        if operation & 0x80 != 0 {
            let mut offset = 0usize;
            let mut size = 0usize;
            for bit in 0..4 {
                if operation & (1 << bit) != 0 {
                    let byte = *delta.get(at).ok_or_else(|| damaged("a delta ends inside a copy instruction"))?;
                    at += 1;
                    offset |= usize::from(byte) << (8 * bit);
                }
            }
            for bit in 0..3 {
                if operation & (0x10 << bit) != 0 {
                    let byte = *delta.get(at).ok_or_else(|| damaged("a delta ends inside a copy instruction"))?;
                    at += 1;
                    size |= usize::from(byte) << (8 * bit);
                }
            }
            if size == 0 {
                size = 0x10000;
            }
            let end = offset
                .checked_add(size)
                .filter(|end| *end <= base.len())
                .ok_or_else(|| damaged("a delta copies from outside its base"))?;
            out.extend_from_slice(&base[offset..end]);
        } else if operation != 0 {
            let end = at + usize::from(operation);
            let inserted = delta.get(at..end).ok_or_else(|| damaged("a delta ends inside an insert instruction"))?;
            out.extend_from_slice(inserted);
            at = end;
        } else {
            return Err(damaged("a delta uses a reserved instruction"));
        }
        if out.len() > wanted {
            return Err(damaged("a delta produces more than it declares"));
        }
    }
    if out.len() != wanted {
        return Err(damaged("a delta produces less than it declares"));
    }
    Ok(out)
}

fn be32(data: &[u8], at: usize) -> Option<u32> {
    let bytes = data.get(at..at.checked_add(4)?)?;
    Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

pub struct PackIndex {
    count: usize,
    data: Vec<u8>,
}

impl PackIndex {
    pub fn parse(data: Vec<u8>) -> Result<PackIndex> {
        if data.len() < INDEX_HEADER_BYTES || data[..4] != [0xff, b't', b'O', b'c'] || be32(&data, 4) != Some(2) {
            return Err(damaged("a pack index is not in version 2 format"));
        }
        let mut previous = 0;
        for slot in 0..256 {
            let total = be32(&data, 8 + slot * 4).unwrap_or(0);
            if total < previous {
                return Err(damaged("a pack index has a fan-out table that is out of order"));
            }
            previous = total;
        }
        let count = usize::try_from(previous).map_err(|_| damaged("a pack index is too large"))?;
        let needed = count.checked_mul(ID_LENGTH + 8).and_then(|tables| tables.checked_add(INDEX_HEADER_BYTES));
        if needed.is_none_or(|needed| data.len() < needed) {
            return Err(damaged("a pack index is shorter than its tables"));
        }
        Ok(PackIndex { count, data })
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    fn fanout(&self, slot: usize) -> usize {
        be32(&self.data, 8 + slot * 4).and_then(|total| usize::try_from(total).ok()).unwrap_or(0)
    }

    fn id_at(&self, index: usize) -> &[u8] {
        let start = INDEX_HEADER_BYTES + index * ID_LENGTH;
        &self.data[start..start + ID_LENGTH]
    }

    fn offset_at(&self, index: usize) -> Option<u64> {
        let table = INDEX_HEADER_BYTES + self.count * (ID_LENGTH + 4);
        let raw = be32(&self.data, table + index * 4)?;
        if raw & 0x8000_0000 == 0 {
            return Some(u64::from(raw));
        }
        let slot = usize::try_from(raw & 0x7fff_ffff).ok()?;
        let large = (table + self.count * 4).checked_add(slot.checked_mul(8)?)?;
        let bytes = self.data.get(large..large.checked_add(8)?)?;
        Some(u64::from_be_bytes(<[u8; 8]>::try_from(bytes).ok()?))
    }

    pub fn find(&self, id: &ObjectId) -> Option<u64> {
        let first = usize::from(id.0[0]);
        let mut low = if first == 0 { 0 } else { self.fanout(first - 1) };
        let mut high = self.fanout(first).min(self.count);
        while low < high {
            let middle = low + (high - low) / 2;
            match self.id_at(middle).cmp(&id.0[..]) {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => return self.offset_at(middle),
            }
        }
        None
    }
}

struct Pack {
    index: PackIndex,
    file: File,
    size: u64,
}

enum Entry {
    Whole(ObjectKind, Vec<u8>),
    OffsetDelta(u64, Vec<u8>),
    ReferenceDelta(ObjectId, Vec<u8>),
}

fn next_byte(reader: &mut impl Read) -> Faulty<u8> {
    let mut byte = [0u8; 1];
    reader.read_exact(&mut byte).map_err(|_| broken("a pack file ends inside an object"))?;
    Ok(byte[0])
}

fn inflate(reader: impl Read, size: u64) -> Faulty<Vec<u8>> {
    let mut out = Vec::new();
    ZlibDecoder::new(reader)
        .take(size.saturating_add(1))
        .read_to_end(&mut out)
        .map_err(|error| Fault::Broken(format!("compressed data cannot be read ({error})")))?;
    if out.len() as u64 != size {
        return Err(broken("an object has another size than its header says"));
    }
    Ok(out)
}

fn read_entry(pack: &mut Pack, offset: u64, limit: u64) -> Faulty<Entry> {
    if offset < PACK_HEADER_BYTES || offset >= pack.size {
        return Err(broken("an object lies outside its pack file"));
    }
    pack.file
        .seek(SeekFrom::Start(offset))
        .map_err(|error| Fault::Broken(format!("the pack file cannot be read ({error})")))?;
    let mut reader = BufReader::with_capacity(16 * 1024, &mut pack.file);
    let mut byte = next_byte(&mut reader)?;
    let code = (byte >> 4) & 7;
    let mut size = u64::from(byte & 0x0f);
    let mut shift = 4u32;
    while byte & 0x80 != 0 {
        byte = next_byte(&mut reader)?;
        if shift > 57 {
            return Err(broken("an object has an unreadable size"));
        }
        size |= u64::from(byte & 0x7f) << shift;
        shift += 7;
    }
    if size > limit {
        return Err(Fault::TooLarge);
    }
    match code {
        6 => {
            let mut byte = next_byte(&mut reader)?;
            let mut distance = u64::from(byte & 0x7f);
            while byte & 0x80 != 0 {
                byte = next_byte(&mut reader)?;
                distance = distance
                    .checked_add(1)
                    .and_then(|value| value.checked_mul(128))
                    .map(|value| value | u64::from(byte & 0x7f))
                    .ok_or_else(|| broken("a delta has an unreadable base offset"))?;
            }
            let base = offset
                .checked_sub(distance)
                .filter(|base| *base >= PACK_HEADER_BYTES && *base < offset)
                .ok_or_else(|| broken("a delta names a base outside its pack file"))?;
            Ok(Entry::OffsetDelta(base, inflate(reader, size)?))
        }
        7 => {
            let mut id = [0u8; ID_LENGTH];
            reader.read_exact(&mut id).map_err(|_| broken("a pack file ends inside an object"))?;
            Ok(Entry::ReferenceDelta(ObjectId(id), inflate(reader, size)?))
        }
        other => {
            let kind = ObjectKind::from_code(other).ok_or_else(|| broken("an object has an unknown type"))?;
            Ok(Entry::Whole(kind, inflate(reader, size)?))
        }
    }
}

fn read_loose(path: &Path, limit: u64) -> Faulty<(ObjectKind, Vec<u8>)> {
    let file = File::open(path).map_err(|error| Fault::Broken(format!("the object file cannot be opened ({error})")))?;
    let ceiling = limit.saturating_add(64);
    let mut data = Vec::new();
    ZlibDecoder::new(BufReader::new(file))
        .take(ceiling)
        .read_to_end(&mut data)
        .map_err(|error| Fault::Broken(format!("compressed data cannot be read ({error})")))?;
    if data.len() as u64 >= ceiling {
        return Err(Fault::TooLarge);
    }
    let (kind, body) = split_loose(&data)?;
    if body.len() as u64 > limit {
        return Err(Fault::TooLarge);
    }
    Ok((kind, body.to_vec()))
}

fn read_small(path: &Path, limit: u64) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(limit).read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn not_a_repository(root: &Path) -> AppError {
    AppError::invalid(
        "path",
        format!("\"{}\" is not a Git repository", root.display()),
        "Name a directory that contains a .git directory, or a bare repository",
    )
}

fn join_plain(base: &Path, relative: &str) -> PathBuf {
    let addition = Path::new(relative);
    if addition.is_absolute() {
        return addition.to_path_buf();
    }
    let mut joined = base.to_path_buf();
    for part in addition.components() {
        match part {
            std::path::Component::ParentDir => {
                joined.pop();
            }
            std::path::Component::CurDir => {}
            other => joined.push(other),
        }
    }
    joined
}

pub fn locate(root: &Path) -> Result<PathBuf> {
    let dot = root.join(".git");
    if dot.is_dir() {
        return Ok(dot);
    }
    if dot.is_file() {
        let text = read_small(&dot, SMALL_FILE_BYTES).unwrap_or_default();
        let target = text.trim().strip_prefix("gitdir:").map(str::trim).ok_or_else(|| not_a_repository(root))?;
        let directory = join_plain(root, target);
        if directory.is_dir() {
            return Ok(directory);
        }
        return Err(not_a_repository(root));
    }
    if root.join("HEAD").is_file() && root.join("objects").is_dir() {
        return Ok(root.to_path_buf());
    }
    Err(not_a_repository(root))
}

fn uses_long_names(config: &str) -> bool {
    config.lines().any(|line| {
        let compact: String = line.chars().filter(|letter| !letter.is_whitespace()).collect::<String>().to_lowercase();
        compact == "objectformat=sha256"
    })
}

pub struct Repository {
    git_dir: PathBuf,
    common_dir: PathBuf,
    object_dirs: Vec<PathBuf>,
    packs: Vec<Pack>,
    skipped_packs: u64,
    cache: HashMap<(usize, u64), (ObjectKind, Vec<u8>)>,
    cached_bytes: usize,
}

fn open_pack(index_path: &Path) -> Option<Pack> {
    let length = fs::metadata(index_path).ok()?.len();
    if length > MAX_INDEX_BYTES {
        return None;
    }
    let index = PackIndex::parse(fs::read(index_path).ok()?).ok()?;
    let mut file = File::open(index_path.with_extension("pack")).ok()?;
    let size = file.metadata().ok()?.len();
    let mut header = [0u8; 12];
    file.read_exact(&mut header).ok()?;
    let version = be32(&header, 4)?;
    if header[..4] != *b"PACK" || !(version == 2 || version == 3) {
        return None;
    }
    Some(Pack { index, file, size })
}

impl Repository {
    pub fn open(root: &Path) -> Result<Repository> {
        let git_dir = locate(root)?;
        let common_dir = match read_small(&git_dir.join("commondir"), SMALL_FILE_BYTES) {
            Some(text) if !text.trim().is_empty() => join_plain(&git_dir, text.trim()),
            _ => git_dir.clone(),
        };
        if uses_long_names(&read_small(&common_dir.join("config"), SMALL_FILE_BYTES).unwrap_or_default()) {
            return Err(AppError::invalid(
                "path",
                "the repository uses SHA-256 object names, which this version cannot read",
                "Scan the files of the working tree instead",
            ));
        }
        let objects = common_dir.join("objects");
        let mut object_dirs = vec![objects.clone()];
        if let Some(text) = read_small(&objects.join("info").join("alternates"), SMALL_FILE_BYTES) {
            for line in text.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')) {
                if object_dirs.len() > MAX_ALTERNATES {
                    break;
                }
                let other = join_plain(&objects, line);
                if other.is_dir() {
                    object_dirs.push(other);
                }
            }
        }
        let mut packs = Vec::new();
        let mut skipped_packs = 0;
        for directory in &object_dirs {
            let Ok(listing) = fs::read_dir(directory.join("pack")) else {
                continue;
            };
            let mut indexes: Vec<PathBuf> = listing
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .filter(|path| path.extension().is_some_and(|extension| extension == "idx"))
                .collect();
            indexes.sort();
            for index_path in indexes {
                if packs.len() >= MAX_PACKS {
                    skipped_packs += 1;
                    continue;
                }
                match open_pack(&index_path) {
                    Some(pack) => packs.push(pack),
                    None => skipped_packs += 1,
                }
            }
        }
        Ok(Repository { git_dir, common_dir, object_dirs, packs, skipped_packs, cache: HashMap::new(), cached_bytes: 0 })
    }

    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    pub fn pack_count(&self) -> usize {
        self.packs.len()
    }

    pub fn packed_object_count(&self) -> usize {
        self.packs.iter().map(|pack| pack.index.len()).sum()
    }

    pub fn skipped_packs(&self) -> u64 {
        self.skipped_packs
    }

    fn find_packed(&self, id: &ObjectId) -> Option<(usize, u64)> {
        self.packs.iter().enumerate().find_map(|(number, pack)| pack.index.find(id).map(|offset| (number, offset)))
    }

    fn loose_path(&self, id: &ObjectId) -> Option<PathBuf> {
        let hex = id.to_hex();
        self.object_dirs.iter().map(|directory| directory.join(&hex[..2]).join(&hex[2..])).find(|path| path.is_file())
    }

    fn remember(&mut self, place: (usize, u64), kind: ObjectKind, data: &[u8]) {
        if data.len() > CACHE_ITEM_BYTES {
            return;
        }
        if self.cached_bytes + data.len() > CACHE_BYTES {
            self.cache.clear();
            self.cached_bytes = 0;
        }
        if self.cache.insert(place, (kind, data.to_vec())).is_none() {
            self.cached_bytes += data.len();
        }
    }

    fn fetch_packed(&mut self, start: (usize, u64), limit: u64) -> Faulty<(ObjectKind, Vec<u8>)> {
        let mut deltas: Vec<Vec<u8>> = Vec::new();
        let mut delta_bytes = 0u64;
        let mut place = start;
        let (kind, mut data) = loop {
            if let Some((kind, data)) = self.cache.get(&place) {
                break (*kind, data.clone());
            }
            if deltas.len() >= MAX_DELTA_DEPTH {
                return Err(broken("a delta chain is longer than the limit"));
            }
            let entry_limit = if deltas.is_empty() { limit } else { MAX_OBJECT_BYTES };
            let (packed, named, delta) = match read_entry(&mut self.packs[place.0], place.1, entry_limit)? {
                Entry::Whole(kind, data) => break (kind, data),
                Entry::OffsetDelta(base, delta) => (Some((place.0, base)), None, delta),
                Entry::ReferenceDelta(id, delta) => (self.find_packed(&id), Some(id), delta),
            };
            if deltas.is_empty() && delta_result_size(&delta)? > limit {
                return Err(Fault::TooLarge);
            }
            delta_bytes += delta.len() as u64;
            if delta_bytes > 2 * MAX_OBJECT_BYTES {
                return Err(broken("a delta chain is larger than the limit"));
            }
            deltas.push(delta);
            match (packed, named) {
                (Some(found), _) => place = found,
                (None, Some(id)) => {
                    let path = self.loose_path(&id).ok_or_else(|| broken("the base of a delta is missing"))?;
                    break read_loose(&path, MAX_OBJECT_BYTES)?;
                }
                (None, None) => return Err(broken("a delta has no base")),
            }
        };
        for delta in deltas.iter().rev() {
            data = apply_delta(&data, delta, MAX_OBJECT_BYTES)?;
        }
        if data.len() as u64 > limit {
            return Err(Fault::TooLarge);
        }
        self.remember(start, kind, &data);
        Ok((kind, data))
    }

    pub fn read(&mut self, id: &ObjectId, limit: u64) -> Result<Fetched> {
        let limit = limit.min(MAX_OBJECT_BYTES);
        let fetched = match self.find_packed(id) {
            Some(place) => self.fetch_packed(place, limit).map(Some),
            None => match self.loose_path(id) {
                Some(path) => read_loose(&path, limit).map(Some),
                None => Ok(None),
            },
        };
        match fetched {
            Ok(Some((kind, data))) => Ok(Fetched::Found(kind, data)),
            Ok(None) => Ok(Fetched::Missing),
            Err(Fault::TooLarge) => Ok(Fetched::TooLarge),
            Err(Fault::Broken(what)) => Err(AppError::Io(format!("Git object {} cannot be read: {what}", id.to_hex()))),
        }
    }

    fn loose_references(directory: &Path, name: &str, depth: usize, found: &mut Vec<(String, String)>) {
        if depth > MAX_REFERENCE_DEPTH || found.len() >= MAX_REFERENCES {
            return;
        }
        let Ok(listing) = fs::read_dir(directory) else {
            return;
        };
        let mut names: Vec<std::ffi::OsString> = listing.filter_map(|entry| entry.ok().map(|entry| entry.file_name())).collect();
        names.sort();
        for file_name in names {
            let path = directory.join(&file_name);
            let child = format!("{name}/{}", file_name.to_string_lossy());
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.is_dir() {
                Repository::loose_references(&path, &child, depth + 1, found);
            } else if metadata.is_file()
                && found.len() < MAX_REFERENCES
                && let Some(text) = read_small(&path, SMALL_FILE_BYTES)
            {
                found.push((child, text.trim().to_string()));
            }
        }
    }

    pub fn references(&self) -> Vec<Reference> {
        let mut resolved: BTreeMap<String, ObjectId> = BTreeMap::new();
        if let Some(text) = read_small(&self.common_dir.join("packed-refs"), PACKED_REFERENCES_BYTES) {
            for line in text.lines() {
                if resolved.len() >= MAX_REFERENCES {
                    break;
                }
                let parsed = line.split_once(' ').and_then(|(hex, name)| ObjectId::from_hex(hex).map(|id| (name, id)));
                if let Some((name, id)) = parsed {
                    resolved.insert(name.trim().to_string(), id);
                }
            }
        }
        let mut loose = Vec::new();
        Repository::loose_references(&self.common_dir.join("refs"), "refs", 1, &mut loose);
        if let Some(text) = read_small(&self.git_dir.join("HEAD"), SMALL_FILE_BYTES) {
            loose.push(("HEAD".to_string(), text.trim().to_string()));
        }
        let mut symbolic = Vec::new();
        for (name, content) in loose {
            match content.strip_prefix("ref:") {
                Some(target) => symbolic.push((name, target.trim().to_string())),
                None => {
                    if let Some(id) = ObjectId::from_hex(&content) {
                        resolved.insert(name, id);
                    }
                }
            }
        }
        for (name, target) in symbolic {
            if let Some(id) = resolved.get(&target).copied() {
                resolved.insert(name, id);
            }
        }
        resolved.into_iter().take(MAX_REFERENCES).map(|(name, id)| Reference { name, id }).collect()
    }
}

fn path_key(path: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    hasher.finish()
}

struct Planned {
    time: i64,
    tree: ObjectId,
    info: CommitInfo,
}

fn collect_commits(
    repository: &mut Repository,
    options: &HistoryOptions,
    cancel: &AtomicBool,
    stats: &mut HistoryStats,
) -> Vec<Planned> {
    let mut pending: Vec<ObjectId> = repository.references().into_iter().map(|reference| reference.id).collect();
    stats.references = pending.len() as u64;
    pending.reverse();
    let mut known: HashSet<ObjectId> = HashSet::new();
    let mut planned = Vec::new();
    while let Some(id) = pending.pop() {
        if cancel.load(Ordering::Relaxed) {
            stats.cancelled = true;
            break;
        }
        if !known.insert(id) {
            continue;
        }
        if planned.len() as u64 >= options.max_commits {
            stats.limit_reached = true;
            break;
        }
        match repository.read(&id, MAX_OBJECT_BYTES) {
            Ok(Fetched::Found(ObjectKind::Commit, body)) => match parse_commit(&body) {
                Ok(commit) => {
                    pending.extend(commit.parents.iter().copied());
                    planned.push(Planned {
                        time: commit.commit_time,
                        tree: commit.tree,
                        info: CommitInfo { id, author: commit.author, time: commit.author_time, subject: commit.subject },
                    });
                }
                Err(_) => stats.unreadable += 1,
            },
            Ok(Fetched::Found(ObjectKind::Tag, body)) => match parse_tag(&body) {
                Ok(target) => pending.push(target),
                Err(_) => stats.unreadable += 1,
            },
            Ok(Fetched::Found(_, _)) => {}
            Ok(Fetched::Missing) => stats.missing += 1,
            Ok(Fetched::TooLarge) | Err(_) => stats.unreadable += 1,
        }
    }
    planned.sort_by_key(|commit| (commit.time, commit.info.id));
    planned
}

pub fn walk_history(
    repository: &mut Repository,
    options: &HistoryOptions,
    cancel: &AtomicBool,
    visit: &mut dyn FnMut(&CommitInfo, &str, &str),
) -> HistoryStats {
    let mut stats = HistoryStats { skipped_packs: repository.skipped_packs, ..HistoryStats::default() };
    let planned = collect_commits(repository, options, cancel, &mut stats);
    if stats.cancelled {
        return stats;
    }
    let mut seen: HashSet<(ObjectId, u64)> = HashSet::new();
    'commits: for commit in &planned {
        if cancel.load(Ordering::Relaxed) {
            stats.cancelled = true;
            break;
        }
        stats.commits_scanned += 1;
        let mut stack: Vec<(ObjectId, String, usize)> = vec![(commit.tree, String::new(), 1)];
        while let Some((tree_id, prefix, depth)) = stack.pop() {
            if !seen.insert((tree_id, path_key(&prefix))) {
                continue;
            }
            if seen.len() > MAX_VISITED {
                stats.limit_reached = true;
                break 'commits;
            }
            let entries = match repository.read(&tree_id, MAX_OBJECT_BYTES) {
                Ok(Fetched::Found(ObjectKind::Tree, body)) => match parse_tree(&body) {
                    Ok(entries) => entries,
                    Err(_) => {
                        stats.unreadable += 1;
                        continue;
                    }
                },
                Ok(Fetched::Missing) => {
                    stats.missing += 1;
                    continue;
                }
                _ => {
                    stats.unreadable += 1;
                    continue;
                }
            };
            for entry in entries {
                let path = if prefix.is_empty() { entry.name } else { format!("{prefix}/{}", entry.name) };
                if entry.mode == MODE_DIRECTORY {
                    if depth < MAX_TREE_DEPTH {
                        stack.push((entry.id, path, depth + 1));
                    } else {
                        stats.too_deep += 1;
                    }
                    continue;
                }
                if !FILE_MODES.contains(&entry.mode) || !seen.insert((entry.id, path_key(&path))) {
                    continue;
                }
                if cancel.load(Ordering::Relaxed) {
                    stats.cancelled = true;
                    break 'commits;
                }
                match repository.read(&entry.id, options.max_blob_bytes) {
                    Ok(Fetched::Found(ObjectKind::Blob, body)) => {
                        if body.iter().take(BINARY_PROBE_BYTES).any(|byte| *byte == 0) {
                            stats.binary += 1;
                        } else {
                            stats.files_scanned += 1;
                            stats.bytes_scanned += body.len() as u64;
                            visit(&commit.info, &path, &String::from_utf8_lossy(&body));
                        }
                    }
                    Ok(Fetched::TooLarge) => stats.too_large += 1,
                    Ok(Fetched::Missing) => stats.missing += 1,
                    Ok(Fetched::Found(_, _)) | Err(_) => stats.unreadable += 1,
                }
            }
        }
    }
    stats
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::testkit::{
        DeltaStyle, GitFixture, TempDir, git_object_id, make_delta, pack_entry_reference_delta, pack_entry_whole,
    };

    fn id(hex: &str) -> ObjectId {
        ObjectId::from_hex(hex).unwrap()
    }

    fn open(directory: &TempDir) -> Repository {
        Repository::open(directory.path()).unwrap()
    }

    fn refused(root: &Path) -> String {
        match Repository::open(root) {
            Ok(_) => panic!("the repository was opened"),
            Err(error) => error.to_string(),
        }
    }

    fn found(repository: &mut Repository, hex: &str, limit: u64) -> (ObjectKind, Vec<u8>) {
        match repository.read(&id(hex), limit).unwrap() {
            Fetched::Found(kind, data) => (kind, data),
            other => panic!("object {hex} gave {other:?}"),
        }
    }

    fn versions() -> Vec<(&'static str, Vec<u8>)> {
        let mut text = String::from("# Notes\n");
        let mut objects = Vec::new();
        for number in 1..=5 {
            text.push_str(&format!("line {number} of the notes file\n"));
            objects.push(("blob", format!("{text}closing line\n").into_bytes()));
        }
        let ids: Vec<String> = objects.iter().map(|(kind, body)| git_object_id(kind, body)).collect();
        let tree = GitFixture::tree_body(&[("100644", "notes.txt", &ids[4]), ("100644", "old.txt", &ids[0])]);
        let tree_id = git_object_id("tree", &tree);
        objects.push(("tree", tree));
        objects.push(("commit", GitFixture::commit_body(&tree_id, &[], 100, "notes")));
        objects
    }

    #[test]
    fn object_names_round_trip_and_reject_other_text() {
        let text = "0123456789abcdef0123456789abcdef01234567";
        assert_eq!(id(text).to_hex(), text);
        assert_eq!(format!("{:?}", id(text)), text);
        assert_eq!(ObjectId::from_hex(&text.to_uppercase()).unwrap().to_hex(), text);
        assert_eq!(ObjectId::from_bytes(id(text).as_bytes()), Some(id(text)));
        assert_eq!(ObjectId::from_bytes(&[1u8; 19]), None);
        for wrong in ["", "abc", &text[1..], &format!("{text}0"), &"g".repeat(40), &"é".repeat(20)] {
            assert_eq!(ObjectId::from_hex(wrong), None, "{wrong}");
        }
    }

    #[test]
    fn loose_objects_are_read_with_their_type_and_within_the_limit() {
        let directory = TempDir::new("git-loose");
        let fixture = GitFixture::create(directory.path());
        let blob = fixture.blob("hello\n");
        assert_eq!(blob, "ce013625030ba8dba906f756967f9e9ca394464a");
        let tree = fixture.tree(&[("100644", "hello.txt", &blob)]);
        let mut repository = open(&directory);
        assert_eq!(found(&mut repository, &blob, 100), (ObjectKind::Blob, b"hello\n".to_vec()));
        assert_eq!(found(&mut repository, &blob, 6).1.len(), 6);
        assert_eq!(repository.read(&id(&blob), 5).unwrap(), Fetched::TooLarge);
        assert_eq!(found(&mut repository, &tree, 1000).0, ObjectKind::Tree);
        assert_eq!(repository.read(&id(&"ab".repeat(20)), 100).unwrap(), Fetched::Missing);
        assert_eq!((repository.pack_count(), repository.packed_object_count(), repository.skipped_packs()), (0, 0, 0));

        fs::write(fixture.object_path(&blob), b"not compressed at all").unwrap();
        let garbage = repository.read(&id(&blob), 100).unwrap_err().to_string();
        assert!(garbage.starts_with(&format!("Git object {blob} cannot be read: ")), "{garbage}");
    }

    #[test]
    fn loose_headers_are_checked() {
        assert_eq!(split_loose(b"blob 3\0abc").unwrap(), (ObjectKind::Blob, &b"abc"[..]));
        assert_eq!(split_loose(b"commit 0\0").unwrap(), (ObjectKind::Commit, &b""[..]));
        for wrong in [&b"blob 3abc"[..], b"blob 4\0abc", b"blob\0abc", b"note 3\0abc", b"blob x\0abc", b"", b"\xff 3\0abc"] {
            let problem = split_loose(wrong).unwrap_err().to_string();
            assert!(problem.starts_with("the Git data is damaged: "), "{problem}");
        }
    }

    #[test]
    fn commits_and_tags_are_parsed_and_damage_is_named() {
        let tree = "1".repeat(40);
        let (first, second) = ("2".repeat(40), "3".repeat(40));
        let body = format!(
            "tree {tree}\nparent {first}\nparent {second}\nauthor Ünï Coder <coder> 1700000100 +0200\ncommitter Other Person <other> 1700000200 -0500\ngpgsig -----BEGIN PGP SIGNATURE-----\n \n tree of signatures\n -----END PGP SIGNATURE-----\n\n\n  Add the parser  \n\nLonger text.\n"
        );
        let commit = parse_commit(body.as_bytes()).unwrap();
        assert_eq!(commit.tree, id(&tree));
        assert_eq!(commit.parents, vec![id(&first), id(&second)]);
        assert_eq!((commit.author.as_str(), commit.author_time, commit.commit_time), ("Ünï Coder", 1_700_000_100, 1_700_000_200));
        assert_eq!(commit.subject, "Add the parser");

        let bare = parse_commit(format!("tree {tree}").as_bytes()).unwrap();
        assert_eq!((bare.parents.len(), bare.author.as_str(), bare.subject.as_str(), bare.commit_time), (0, "", "", 0));
        let odd = parse_commit(format!("tree {tree}\nauthor no bracket\ncommitter <x> soon\n\n\u{7}bell\n").as_bytes()).unwrap();
        assert_eq!((odd.author.as_str(), odd.author_time, odd.commit_time, odd.subject.as_str()), ("no bracket", 0, 0, "bell"));

        assert!(parse_commit(b"author nobody\n\nno tree\n").unwrap_err().to_string().ends_with("a commit has no tree"));
        assert!(parse_commit(b"tree xyz\n").unwrap_err().to_string().ends_with("a commit names an unreadable tree"));
        assert!(parse_commit(format!("tree {tree}\nparent 12\n").as_bytes()).is_err());
        let crowded = format!("tree {tree}\n{}", format!("parent {first}\n").repeat(MAX_PARENTS + 1));
        assert!(parse_commit(crowded.as_bytes()).unwrap_err().to_string().ends_with("a commit has too many parents"));

        let tag = format!("object {first}\ntype commit\ntag v1\n\nobject {second}\n");
        assert_eq!(parse_tag(tag.as_bytes()).unwrap(), id(&first));
        assert!(parse_tag(b"type commit\n\nobject none\n").is_err());
        assert!(parse_tag(b"object short\n").is_err());
    }

    #[test]
    fn trees_are_parsed_and_damage_is_named() {
        let (file, folder, link) = ("a".repeat(40), "b".repeat(40), "c".repeat(40));
        let body = GitFixture::tree_body(&[("100644", "read me.txt", &file), ("40000", "src", &folder), ("120000", "link", &link)]);
        let entries = parse_tree(&body).unwrap();
        assert_eq!(
            entries.iter().map(|entry| (entry.mode, entry.name.as_str(), entry.id)).collect::<Vec<_>>(),
            vec![(0o120000, "link", id(&link)), (0o100644, "read me.txt", id(&file)), (MODE_DIRECTORY, "src", id(&folder))]
        );
        assert_eq!(parse_tree(b"").unwrap(), Vec::new());
        let mut latin = b"100644 caf\xe9\0".to_vec();
        latin.extend([7u8; 20]);
        assert_eq!(parse_tree(&latin).unwrap()[0].name, "caf\u{fffd}");

        let problems: Vec<(Vec<u8>, &str)> = vec![
            (body[..body.len() - 1].to_vec(), "a tree entry ends inside its object name"),
            (b"100644 name-without-end".to_vec(), "a tree entry has no name"),
            (b"100644name\0".to_vec(), "a tree entry has no mode"),
            (b"10x644 name\0".to_vec(), "a tree entry has an unreadable mode"),
            (b" name\0".to_vec(), "a tree entry has an unreadable mode"),
            ([&b"100644 a/b\0"[..], &[1u8; 20]].concat(), "a tree entry has an unusable name"),
            ([&b"100644 ..\0"[..], &[1u8; 20]].concat(), "a tree entry has an unusable name"),
            ([&b"100644 \0"[..], &[1u8; 20]].concat(), "a tree entry has an unusable name"),
        ];
        for (damaged, expected) in problems {
            let problem = parse_tree(&damaged).unwrap_err().to_string();
            assert_eq!(problem, format!("the Git data is damaged: {expected}"));
        }
    }

    #[test]
    fn deltas_rebuild_the_object_and_damage_is_named() {
        let base = b"line one\nline two\nline three\n";
        let target = b"line one\nline 2\nline three\nline four\n";
        let delta = make_delta(base, target);
        assert_eq!(apply_delta(base, &delta, 1000).unwrap(), target);
        assert_eq!(delta_result_size(&delta).unwrap(), target.len() as u64);
        assert_eq!(apply_delta(base, &make_delta(base, b""), 1000).unwrap(), b"");
        assert_eq!(apply_delta(b"", &make_delta(b"", target), 1000).unwrap(), target);
        assert_eq!(apply_delta(base, &make_delta(base, base), 1000).unwrap(), base);

        let block = vec![b'q'; 0x10000];
        assert_eq!(apply_delta(&block, &[0x80, 0x80, 0x04, 0x80, 0x80, 0x04, 0x80], 0x10000).unwrap(), block);
        let grown = [&block[..], b"tail"].concat();
        assert_eq!(apply_delta(&block, &make_delta(&block, &grown), 1 << 20).unwrap(), grown);

        let problems: Vec<(&[u8], Vec<u8>, &str)> = vec![
            (b"other", delta.clone(), "a delta was made for another base"),
            (base, Vec::new(), "a delta ends inside its header"),
            (base, vec![0x80], "a delta ends inside its header"),
            (b"a", vec![1, 1, 0], "a delta uses a reserved instruction"),
            (b"abcd", vec![4, 4, 0x91, 2, 4], "a delta copies from outside its base"),
            (b"abcd", vec![4, 4, 0x91], "a delta ends inside a copy instruction"),
            (b"", vec![0, 5, 5, b'a'], "a delta ends inside an insert instruction"),
            (b"", vec![0, 5, 2, b'a', b'b'], "a delta produces less than it declares"),
            (b"", vec![0, 1, 2, b'a', b'b'], "a delta produces more than it declares"),
            (b"", vec![0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01], "a delta has an unreadable size"),
        ];
        for (base, damaged, expected) in problems {
            let problem = apply_delta(base, &damaged, 1000).unwrap_err().to_string();
            assert_eq!(problem, format!("the Git data is damaged: {expected}"));
        }
        let large = apply_delta(base, &delta, 10).unwrap_err().to_string();
        assert!(large.ends_with("a delta produces an object above the size limit"), "{large}");
    }

    #[test]
    fn packed_objects_are_read_whole_and_through_delta_chains() {
        for style in [DeltaStyle::None, DeltaStyle::Offset, DeltaStyle::Reference] {
            let directory = TempDir::new("git-pack");
            let fixture = GitFixture::create(directory.path());
            let objects = versions();
            let ids = fixture.pack("pack-sample", &objects, style);
            let mut repository = open(&directory);
            assert_eq!((repository.pack_count(), repository.packed_object_count()), (1, objects.len()), "{style:?}");
            for round in 0..2 {
                for ((kind, body), hex) in objects.iter().zip(&ids) {
                    let (read_kind, read_body) = found(&mut repository, hex, 10_000);
                    assert_eq!(ObjectKind::parse(kind), Some(read_kind), "{style:?} round {round}");
                    assert_eq!(&read_body, body, "{style:?} round {round} object {hex}");
                }
            }
            assert_eq!(repository.read(&id(&ids[4]), 20).unwrap(), Fetched::TooLarge, "{style:?}");
            assert_eq!(repository.read(&id(&ids[0]), 20).unwrap(), Fetched::TooLarge, "{style:?}");
            assert_eq!(repository.read(&id(&"cd".repeat(20)), 100).unwrap(), Fetched::Missing);
            let mut fresh = open(&directory);
            assert_eq!(found(&mut fresh, &ids[4], 10_000).1, objects[4].1, "{style:?}");
        }
    }

    #[test]
    fn a_delta_may_have_its_base_outside_the_pack() {
        let directory = TempDir::new("git-thin");
        let fixture = GitFixture::create(directory.path());
        let (old, new) = (b"shared start\nold end\n".to_vec(), b"shared start\nnew end\n".to_vec());
        let base = fixture.object("blob", &old);
        let target = git_object_id("blob", &new);
        fixture.write_pack("pack-thin", &[(target.clone(), pack_entry_reference_delta(&base, &make_delta(&old, &new)))]);
        let mut repository = open(&directory);
        assert_eq!(found(&mut repository, &target, 100), (ObjectKind::Blob, new));

        fs::remove_file(fixture.object_path(&base)).unwrap();
        let mut repository = open(&directory);
        let missing = repository.read(&id(&target), 100).unwrap_err().to_string();
        assert!(missing.ends_with("the base of a delta is missing"), "{missing}");
    }

    #[test]
    fn damaged_packs_are_refused_without_a_panic() {
        let directory = TempDir::new("git-damaged");
        let fixture = GitFixture::create(directory.path());
        let (first, second) = (git_object_id("blob", b"first"), git_object_id("blob", b"second"));
        let ring = vec![
            (first.clone(), pack_entry_reference_delta(&second, &make_delta(b"second", b"first"))),
            (second.clone(), pack_entry_reference_delta(&first, &make_delta(b"first", b"second"))),
        ];
        fixture.write_pack("pack-ring", &ring);
        let mut repository = open(&directory);
        let endless = repository.read(&id(&first), 100).unwrap_err().to_string();
        assert!(endless.ends_with("a delta chain is longer than the limit"), "{endless}");

        let whole = git_object_id("blob", b"whole object in a short pack");
        fixture.write_pack("pack-short", &[(whole.clone(), pack_entry_whole("blob", b"whole object in a short pack"))]);
        let pack_path = fixture.git_dir().join("objects").join("pack").join("pack-short.pack");
        let bytes = fs::read(&pack_path).unwrap();
        fs::write(&pack_path, &bytes[..20]).unwrap();
        let mut repository = open(&directory);
        assert!(repository.read(&id(&whole), 100).is_err());
        fs::write(&pack_path, &bytes[..12]).unwrap();
        let mut repository = open(&directory);
        let outside = repository.read(&id(&whole), 100).unwrap_err().to_string();
        assert!(outside.ends_with("an object lies outside its pack file"), "{outside}");

        let pack_directory = fixture.git_dir().join("objects").join("pack");
        fs::write(pack_directory.join("pack-noise.idx"), b"this is not an index").unwrap();
        fs::write(pack_directory.join("pack-noise.pack"), b"this is not a pack").unwrap();
        fs::copy(pack_directory.join("pack-ring.idx"), pack_directory.join("pack-alone.idx")).unwrap();
        fs::copy(pack_directory.join("pack-ring.idx"), pack_directory.join("pack-wrong.idx")).unwrap();
        fs::write(pack_directory.join("pack-wrong.pack"), b"JUNK\0\0\0\x02\0\0\0\0").unwrap();
        let repository = open(&directory);
        assert_eq!((repository.pack_count(), repository.skipped_packs()), (2, 3));
    }

    #[test]
    fn a_pack_index_is_validated_and_finds_large_offsets() {
        let name = [0x5au8; 20];
        let mut index = vec![0xff, b't', b'O', b'c', 0, 0, 0, 2];
        for slot in 0..256 {
            index.extend(u32::from(slot >= 0x5a).to_be_bytes());
        }
        index.extend(name);
        index.extend([0u8; 4]);
        index.extend(0x8000_0000u32.to_be_bytes());
        index.extend(0x0000_0001_0000_0010u64.to_be_bytes());
        let parsed = PackIndex::parse(index.clone()).ok().unwrap();
        assert_eq!((parsed.len(), parsed.is_empty()), (1, false));
        assert_eq!(parsed.find(&ObjectId(name)), Some(0x0000_0001_0000_0010));
        assert_eq!(parsed.find(&ObjectId([0x5bu8; 20])), None);
        assert_eq!(parsed.find(&ObjectId([0u8; 20])), None);
        assert_eq!(parsed.find(&ObjectId([0xffu8; 20])), None);

        let mut small = index.clone();
        let entry = small.len() - 12;
        small[entry..entry + 4].copy_from_slice(&77u32.to_be_bytes());
        assert_eq!(PackIndex::parse(small).ok().unwrap().find(&ObjectId(name)), Some(77));
        let mut dangling = index.clone();
        dangling.truncate(index.len() - 8);
        assert_eq!(PackIndex::parse(dangling).ok().unwrap().find(&ObjectId(name)), None);

        let problem = |bytes: Vec<u8>| PackIndex::parse(bytes).err().map(|error| error.to_string()).unwrap_or_default();
        assert!(problem(index[..100].to_vec()).ends_with("a pack index is not in version 2 format"));
        let mut old = index.clone();
        old[7] = 1;
        assert!(problem(old).ends_with("a pack index is not in version 2 format"));
        let mut disordered = index.clone();
        disordered[8 + 0x60 * 4 + 3] = 0;
        assert!(problem(disordered).ends_with("a pack index has a fan-out table that is out of order"));
        assert!(problem(index[..INDEX_HEADER_BYTES + 10].to_vec()).ends_with("a pack index is shorter than its tables"));
        let empty = PackIndex::parse([&[0xff, b't', b'O', b'c', 0, 0, 0, 2][..], &[0u8; 1024]].concat()).ok().unwrap();
        assert!(empty.is_empty());
        assert_eq!(empty.find(&ObjectId(name)), None);
    }

    #[test]
    fn references_come_from_files_and_the_packed_list() {
        let directory = TempDir::new("git-refs");
        let fixture = GitFixture::create(directory.path());
        let tree = fixture.tree(&[]);
        let first = fixture.commit(&tree, &[], 100, "one");
        let second = fixture.commit(&tree, &[&first], 200, "two");
        fixture.set_reference("refs/heads/main", &second);
        fixture.set_reference("refs/heads/feature/login", &first);
        fixture.set_reference("refs/heads/broken", "not an object name");
        fixture.set_reference("refs/remotes/origin/HEAD", "ref: refs/remotes/origin/main");
        let packed = format!(
            "# pack-refs with: peeled fully-peeled sorted \n{first} refs/heads/main\n{first} refs/remotes/origin/main\n{second} refs/tags/v1\n^{first}\nshort line\n"
        );
        fs::write(fixture.git_dir().join("packed-refs"), packed).unwrap();
        let listed = open(&directory).references();
        let shown: Vec<(&str, String)> = listed.iter().map(|reference| (reference.name.as_str(), reference.id.to_hex())).collect();
        assert_eq!(
            shown,
            vec![
                ("HEAD", second.clone()),
                ("refs/heads/feature/login", first.clone()),
                ("refs/heads/main", second.clone()),
                ("refs/remotes/origin/HEAD", first.clone()),
                ("refs/remotes/origin/main", first.clone()),
                ("refs/tags/v1", second.clone()),
            ]
        );

        fixture.set_reference("HEAD", &first);
        assert_eq!(open(&directory).references()[0], Reference { name: "HEAD".to_string(), id: id(&first) });
        fixture.set_reference("HEAD", "ref: refs/heads/unborn");
        assert_eq!(open(&directory).references()[0].name, "refs/heads/feature/login");
    }

    struct History {
        directory: TempDir,
        fixture: GitFixture,
        first: String,
    }

    fn history() -> History {
        let directory = TempDir::new("git-history");
        let fixture = GitFixture::create(directory.path());
        let alpha_one = fixture.blob("alpha v1\n");
        let alpha_two = fixture.blob("alpha v2\n");
        let beta = fixture.blob("beta\n");
        let gamma = fixture.blob("gamma\n");
        let nested = fixture.blob("nested\n");
        let binary = fixture.object("blob", b"bin\0ary");
        let big = fixture.blob(&"x".repeat(200));
        let folder = fixture.tree(&[("100644", "s.txt", &nested), ("100644", "data.bin", &binary), ("100755", "big.txt", &big)]);
        let tree_one = fixture.tree(&[("100644", "a.txt", &alpha_one)]);
        let tree_two = fixture.tree(&[
            ("100644", "a.txt", &alpha_two),
            ("100644", "b.txt", &beta),
            ("40000", "dir", &folder),
            ("120000", "link", &beta),
            ("160000", "sub", &"9".repeat(40)),
        ]);
        let tree_three = fixture.tree(&[("100644", "a.txt", &alpha_one), ("100644", "b.txt", &beta), ("40000", "dir", &folder)]);
        let tree_side = fixture.tree(&[("100644", "a.txt", &alpha_one), ("100644", "c.txt", &gamma)]);
        let first = fixture.commit(&tree_one, &[], 100, "one");
        let second = fixture.commit(&tree_two, &[&first], 200, "two");
        let third = fixture.commit(&tree_three, &[&second], 300, "three");
        let side = fixture.commit(&tree_side, &[&first], 250, "side");
        fixture.set_reference("refs/heads/main", &third);
        fixture.set_reference("refs/heads/side", &side);
        fixture.set_reference("refs/tags/v1", &fixture.tag(&second, "v1"));
        History { directory, fixture, first }
    }

    fn never(_: &CommitInfo, _: &str, _: &str) {
        panic!("nothing is read");
    }

    fn visits(history: &History, options: &HistoryOptions) -> (Vec<(String, String, String)>, HistoryStats) {
        let mut seen = Vec::new();
        let stats = walk_history(&mut open(&history.directory), options, &AtomicBool::new(false), &mut |commit, path, content| {
            seen.push((commit.subject.clone(), path.to_string(), content.to_string()));
        });
        (seen, stats)
    }

    #[test]
    fn history_is_walked_oldest_first_and_each_file_version_is_visited_once() {
        let history = history();
        let options = HistoryOptions { max_blob_bytes: 100, ..HistoryOptions::default() };
        let (seen, stats) = visits(&history, &options);
        let expected = [
            ("one", "a.txt", "alpha v1\n"),
            ("two", "a.txt", "alpha v2\n"),
            ("two", "b.txt", "beta\n"),
            ("two", "dir/s.txt", "nested\n"),
            ("side", "c.txt", "gamma\n"),
        ];
        assert_eq!(seen.len(), expected.len(), "{seen:?}");
        for (visit, (subject, path, content)) in seen.iter().zip(expected) {
            assert_eq!((visit.0.as_str(), visit.1.as_str(), visit.2.as_str()), (subject, path, content));
        }
        assert_eq!((stats.references, stats.commits_scanned, stats.files_scanned, stats.bytes_scanned), (4, 4, 5, 36));
        assert_eq!((stats.binary, stats.too_large, stats.unreadable, stats.missing, stats.too_deep), (1, 1, 0, 0, 0));
        assert_eq!(stats.skipped(), 2);
        assert!(stats.complete());
        assert_eq!(visits(&history, &options), (seen, stats));

        let mut details = Vec::new();
        walk_history(&mut open(&history.directory), &options, &AtomicBool::new(false), &mut |commit, _, _| {
            details.push(commit.clone());
        });
        assert_eq!(details[0], CommitInfo { id: id(&history.first), author: "Sample Author".to_string(), time: 100, subject: "one".to_string() });
    }

    #[test]
    fn a_history_walk_respects_cancelling_the_commit_limit_and_missing_objects() {
        let history = history();
        let cancelled =
            walk_history(&mut open(&history.directory), &HistoryOptions::default(), &AtomicBool::new(true), &mut never);
        assert!(cancelled.cancelled && !cancelled.complete());
        assert_eq!(cancelled.commits_scanned, 0);

        let cancel = AtomicBool::new(false);
        let mut count = 0;
        let stopped = walk_history(&mut open(&history.directory), &HistoryOptions::default(), &cancel, &mut |_, _, _| {
            count += 1;
            cancel.store(true, Ordering::Relaxed);
        });
        assert_eq!(count, 1);
        assert!(stopped.cancelled);

        let limited = visits(&history, &HistoryOptions { max_commits: 2, ..HistoryOptions::default() }).1;
        assert!(limited.limit_reached && !limited.complete());
        assert_eq!(limited.commits_scanned, 2);

        fs::remove_file(history.fixture.object_path(&history.first)).unwrap();
        let (seen, shallow) = visits(&history, &HistoryOptions::default());
        assert_eq!((shallow.commits_scanned, shallow.missing), (3, 1));
        assert!(shallow.complete());
        assert_eq!(seen[0], ("two".to_string(), "a.txt".to_string(), "alpha v2\n".to_string()));
        assert!(seen.iter().any(|visit| visit.1 == "a.txt" && visit.2 == "alpha v1\n" && visit.0 == "side"));
    }

    #[test]
    fn a_packed_history_gives_the_same_visits_as_loose_objects() {
        let loose = history();
        let expected = visits(&loose, &HistoryOptions::default());
        for style in [DeltaStyle::Offset, DeltaStyle::Reference] {
            let packed = history();
            let mut objects: Vec<(&'static str, Vec<u8>)> = Vec::new();
            let mut files = Vec::new();
            for entry in fs::read_dir(packed.fixture.git_dir().join("objects")).unwrap() {
                let path = entry.unwrap().path();
                if path.file_name().unwrap().len() != 2 {
                    continue;
                }
                for file in fs::read_dir(&path).unwrap() {
                    files.push(file.unwrap().path());
                }
            }
            files.sort();
            for file in &files {
                let (kind, body) = read_loose(file, MAX_OBJECT_BYTES).ok().unwrap();
                let kind = match kind {
                    ObjectKind::Commit => "commit",
                    ObjectKind::Tree => "tree",
                    ObjectKind::Blob => "blob",
                    ObjectKind::Tag => "tag",
                };
                objects.push((kind, body));
            }
            packed.fixture.pack("pack-all", &objects, style);
            for file in &files {
                fs::remove_file(file).unwrap();
            }
            assert_eq!(open(&packed.directory).packed_object_count(), objects.len());
            assert_eq!(visits(&packed, &HistoryOptions::default()), expected, "{style:?}");
        }
    }

    #[test]
    fn a_repository_is_found_in_its_usual_shapes() {
        let directory = TempDir::new("git-shapes");
        let main = directory.path().join("main");
        let fixture = GitFixture::create(&main);
        let blob = fixture.blob("content\n");
        let tree = fixture.tree(&[("100644", "file.txt", &blob)]);
        let commit = fixture.commit(&tree, &[], 100, "only");
        fixture.set_reference("refs/heads/main", &commit);

        let empty = directory.path().join("empty");
        fs::create_dir_all(&empty).unwrap();
        let plain = refused(&empty);
        assert!(plain.starts_with("path: ") && plain.contains("is not a Git repository"), "{plain}");
        assert!(plain.ends_with("Name a directory that contains a .git directory, or a bare repository"), "{plain}");

        let mut bare = Repository::open(fixture.git_dir()).unwrap();
        assert_eq!(bare.git_dir(), fixture.git_dir());
        assert_eq!(found(&mut bare, &blob, 100).1, b"content\n");

        let linked = directory.path().join("linked");
        fs::create_dir_all(&linked).unwrap();
        fs::write(linked.join(".git"), format!("gitdir: {}\n", fixture.git_dir().display())).unwrap();
        assert_eq!(Repository::open(&linked).unwrap().references().len(), 2);
        fs::write(linked.join(".git"), "gitdir: nowhere\n").unwrap();
        assert!(refused(&linked).contains("is not a Git repository"));
        fs::write(linked.join(".git"), "something else\n").unwrap();
        assert!(refused(&linked).contains("is not a Git repository"));

        let worktree_data = fixture.git_dir().join("worktrees").join("second");
        fs::create_dir_all(&worktree_data).unwrap();
        fs::write(worktree_data.join("HEAD"), format!("{commit}\n")).unwrap();
        fs::write(worktree_data.join("commondir"), "../..\n").unwrap();
        let worktree = directory.path().join("second");
        fs::create_dir_all(&worktree).unwrap();
        fs::write(worktree.join(".git"), format!("gitdir: {}\n", worktree_data.display())).unwrap();
        let mut second = Repository::open(&worktree).unwrap();
        assert_eq!(second.references().iter().map(|reference| reference.name.as_str()).collect::<Vec<_>>(), vec!["HEAD", "refs/heads/main"]);
        assert_eq!(found(&mut second, &blob, 100).1, b"content\n");

        let borrower = directory.path().join("borrower");
        let borrowing = GitFixture::create(&borrower);
        fs::create_dir_all(borrowing.git_dir().join("objects").join("info")).unwrap();
        fs::write(
            borrowing.git_dir().join("objects").join("info").join("alternates"),
            format!("# borrowed\n{}\n/no/such/directory\n", fixture.git_dir().join("objects").display()),
        )
        .unwrap();
        assert_eq!(found(&mut Repository::open(&borrower).unwrap(), &blob, 100).1, b"content\n");

        let mut config = fs::read_to_string(fixture.git_dir().join("config")).unwrap();
        config.push_str("[extensions]\n\tobjectFormat = sha256\n");
        fs::write(fixture.git_dir().join("config"), config).unwrap();
        let long = refused(&main);
        assert_eq!(
            long,
            "path: the repository uses SHA-256 object names, which this version cannot read. Scan the files of the working tree instead"
        );
    }

    proptest! {
        #[test]
        fn any_bytes_are_parsed_without_a_panic(data in proptest::collection::vec(any::<u8>(), 0..300), base in proptest::collection::vec(any::<u8>(), 0..64)) {
            let _ = split_loose(&data);
            let _ = parse_commit(&data);
            let _ = parse_tag(&data);
            let _ = parse_tree(&data);
            let _ = delta_result_size(&data);
            if let Ok(result) = apply_delta(&base, &data, 4096) {
                prop_assert!(result.len() <= 4096);
            }
            let mut index = vec![0xff, b't', b'O', b'c', 0, 0, 0, 2];
            index.extend_from_slice(&data);
            index.resize(index.len().max(INDEX_HEADER_BYTES + 64), 1);
            if let Ok(parsed) = PackIndex::parse(index) {
                let _ = parsed.find(&ObjectId([data.first().copied().unwrap_or(0); 20]));
            }
        }

        #[test]
        fn a_delta_between_any_two_texts_rebuilds_the_second(base in proptest::collection::vec(any::<u8>(), 0..400), target in proptest::collection::vec(any::<u8>(), 0..400), shared in proptest::collection::vec(any::<u8>(), 0..200)) {
            let old = [&shared[..], &base[..], &shared[..]].concat();
            let new = [&shared[..], &target[..], &shared[..]].concat();
            let delta = make_delta(&old, &new);
            prop_assert_eq!(delta_result_size(&delta).unwrap(), new.len() as u64);
            prop_assert_eq!(apply_delta(&old, &delta, 4096).unwrap(), new);
        }
    }
}
