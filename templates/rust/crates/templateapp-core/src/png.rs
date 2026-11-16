const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
const LENGTH_BASE: [usize; 29] =
    [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LENGTH_EXTRA: [u32; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DISTANCE_BASE: [usize; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145,
    8193, 12289, 16385, 24577,
];
const DISTANCE_EXTRA: [u32; 30] =
    [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];
const MAX_MATCH: usize = 258;
const MIN_MATCH: usize = 3;
const END_OF_BLOCK: u32 = 256;
const FILTER_NONE: u8 = 0;
const FILTER_SUB: u8 = 1;
const FILTER_UP: u8 = 2;
const PIXEL: usize = 4;

pub fn crc32(parts: &[&[u8]]) -> u32 {
    let mut crc = u32::MAX;
    for part in parts {
        for byte in *part {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = if crc & 1 == 1 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
            }
        }
    }
    !crc
}

pub fn adler32(data: &[u8]) -> u32 {
    let (mut low, mut high) = (1u32, 0u32);
    for byte in data {
        low = (low + u32::from(*byte)) % 65521;
        high = (high + low) % 65521;
    }
    (high << 16) | low
}

#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
    pending: u32,
    count: u32,
}

impl Writer {
    fn push(&mut self, value: u32, count: u32) {
        self.pending |= value << self.count;
        self.count += count;
        while self.count >= 8 {
            self.bytes.push(self.pending as u8);
            self.pending >>= 8;
            self.count -= 8;
        }
    }

    fn code(&mut self, value: u32, count: u32) {
        for bit in (0..count).rev() {
            self.push((value >> bit) & 1, 1);
        }
    }

    fn symbol(&mut self, value: u32) {
        match value {
            0..=143 => self.code(0x30 + value, 8),
            144..=255 => self.code(0x190 + value - 144, 9),
            256..=279 => self.code(value - 256, 7),
            _ => self.code(0xC0 + value - 280, 8),
        }
    }

    fn repeat(&mut self, length: usize, distance: usize) {
        let slot = LENGTH_BASE.iter().rposition(|base| *base <= length).unwrap_or(0);
        self.symbol(257 + slot as u32);
        self.push((length - LENGTH_BASE[slot]) as u32, LENGTH_EXTRA[slot]);
        let slot = DISTANCE_BASE.iter().rposition(|base| *base <= distance).unwrap_or(0);
        self.code(slot as u32, 5);
        self.push((distance - DISTANCE_BASE[slot]) as u32, DISTANCE_EXTRA[slot]);
    }

    fn finish(mut self) -> Vec<u8> {
        if self.count > 0 {
            self.bytes.push(self.pending as u8);
        }
        self.bytes
    }
}

pub fn deflate(data: &[u8]) -> Vec<u8> {
    let mut out = Writer::default();
    out.push(1, 1);
    out.push(1, 2);
    let mut at = 0;
    while at < data.len() {
        let (mut best, mut reach) = (0, 0);
        for distance in [1, PIXEL] {
            if at < distance {
                continue;
            }
            let mut length = 0;
            while length < MAX_MATCH && at + length < data.len() && data[at + length] == data[at + length - distance] {
                length += 1;
            }
            if length > best {
                best = length;
                reach = distance;
            }
        }
        if best >= MIN_MATCH {
            out.repeat(best, reach);
            at += best;
        } else {
            out.symbol(u32::from(data[at]));
            at += 1;
        }
    }
    out.symbol(END_OF_BLOCK);
    out.finish()
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    out.extend_from_slice(&crc32(&[kind, body]).to_be_bytes());
}

pub fn encode(width: usize, height: usize, rgba: &[u8]) -> Vec<u8> {
    let stride = width * PIXEL;
    let mut raw = Vec::with_capacity((stride + 1) * height);
    for row in 0..height {
        raw.push(FILTER_UP);
        for column in 0..stride {
            let above = if row == 0 { 0 } else { rgba[(row - 1) * stride + column] };
            raw.push(rgba[row * stride + column].wrapping_sub(above));
        }
    }
    let mut packed = vec![0x78, 0x01];
    packed.extend(deflate(&raw));
    packed.extend_from_slice(&adler32(&raw).to_be_bytes());
    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&(width as u32).to_be_bytes());
    header.extend_from_slice(&(height as u32).to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    let mut out = SIGNATURE.to_vec();
    chunk(&mut out, b"IHDR", &header);
    chunk(&mut out, b"IDAT", &packed);
    chunk(&mut out, b"IEND", &[]);
    out
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn bit(&mut self) -> Result<u32, String> {
        let byte = self.data.get(self.at / 8).ok_or("the compressed data ends early")?;
        let value = u32::from(*byte >> (self.at % 8)) & 1;
        self.at += 1;
        Ok(value)
    }

    fn bits(&mut self, count: u32) -> Result<u32, String> {
        let mut value = 0;
        for place in 0..count {
            value |= self.bit()? << place;
        }
        Ok(value)
    }

    fn code(&mut self, count: u32) -> Result<u32, String> {
        let mut value = 0;
        for _ in 0..count {
            value = (value << 1) | self.bit()?;
        }
        Ok(value)
    }

    fn symbol(&mut self) -> Result<u32, String> {
        let mut code = self.code(7)?;
        if code <= 0x17 {
            return Ok(256 + code);
        }
        code = (code << 1) | self.bit()?;
        if (0x30..=0xBF).contains(&code) {
            return Ok(code - 0x30);
        }
        if (0xC0..=0xC7).contains(&code) {
            return Ok(280 + code - 0xC0);
        }
        code = (code << 1) | self.bit()?;
        Ok(144 + code - 0x190)
    }
}

pub fn inflate(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut reader = Reader { data, at: 0 };
    let mut out = Vec::new();
    loop {
        let last = reader.bit()? == 1;
        if reader.bits(2)? != 1 {
            return Err("only blocks with fixed codes are supported".to_string());
        }
        loop {
            let symbol = reader.symbol()?;
            if symbol < END_OF_BLOCK {
                out.push(symbol as u8);
                continue;
            }
            if symbol == END_OF_BLOCK {
                break;
            }
            let slot = (symbol - 257) as usize;
            let base = LENGTH_BASE.get(slot).ok_or("the compressed data has an unknown length code")?;
            let length = base + reader.bits(LENGTH_EXTRA[slot])? as usize;
            let slot = reader.code(5)? as usize;
            let base = DISTANCE_BASE.get(slot).ok_or("the compressed data has an unknown distance code")?;
            let distance = base + reader.bits(DISTANCE_EXTRA[slot])? as usize;
            if distance > out.len() {
                return Err("the compressed data points before its start".to_string());
            }
            for _ in 0..length {
                out.push(out[out.len() - distance]);
            }
        }
        if last {
            return Ok(out);
        }
    }
}

fn number(data: &[u8], at: usize) -> Result<usize, String> {
    let bytes = data.get(at..at + 4).ok_or("the picture ends early")?;
    Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize)
}

pub fn decode(data: &[u8]) -> Result<(usize, usize, Vec<u8>), String> {
    if data.len() < SIGNATURE.len() || data[..SIGNATURE.len()] != SIGNATURE {
        return Err("this is not a PNG picture".to_string());
    }
    let (mut width, mut height) = (0, 0);
    let mut packed = Vec::new();
    let mut at = SIGNATURE.len();
    loop {
        let length = number(data, at)?;
        let kind = data.get(at + 4..at + 8).ok_or("the picture ends early")?;
        let body = data.get(at + 8..at + 8 + length).ok_or("the picture ends early")?;
        if number(data, at + 8 + length)? != crc32(&[kind, body]) as usize {
            return Err("a checksum of the picture is wrong".to_string());
        }
        at += 12 + length;
        match kind {
            b"IHDR" => {
                width = number(body, 0)?;
                height = number(body, 4)?;
                if body.get(8..13) != Some(&[8, 6, 0, 0, 0]) {
                    return Err("only 8 bit RGBA pictures without interlacing are supported".to_string());
                }
            }
            b"IDAT" => packed.extend_from_slice(body),
            b"IEND" => break,
            _ => {}
        }
    }
    if packed.len() < 6 || packed[0] != 0x78 {
        return Err("the picture data is not a zlib stream".to_string());
    }
    let raw = inflate(&packed[2..packed.len() - 4])?;
    if number(&packed, packed.len() - 4)? != adler32(&raw) as usize {
        return Err("the checksum of the picture data is wrong".to_string());
    }
    let stride = width * PIXEL;
    if raw.len() != (stride + 1) * height {
        return Err("the picture data has the wrong length".to_string());
    }
    let mut rgba = vec![0u8; stride * height];
    for row in 0..height {
        let filter = raw[row * (stride + 1)];
        for column in 0..stride {
            let value = raw[row * (stride + 1) + 1 + column];
            let before = match filter {
                FILTER_NONE => 0,
                FILTER_SUB if column >= PIXEL => rgba[row * stride + column - PIXEL],
                FILTER_SUB => 0,
                FILTER_UP if row > 0 => rgba[(row - 1) * stride + column],
                FILTER_UP => 0,
                _ => return Err("the picture uses an unsupported row filter".to_string()),
            };
            rgba[row * stride + column] = value.wrapping_add(before);
        }
    }
    Ok((width, height, rgba))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(count: usize) -> Vec<u8> {
        let mut state = 0x2545_F491u32;
        (0..count)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                (state >> 24) as u8
            })
            .collect()
    }

    #[test]
    fn checksums_match_the_published_values() {
        assert_eq!(crc32(&[b"123456789"]), 0xCBF4_3926);
        assert_eq!(crc32(&[b"1234", b"56789"]), 0xCBF4_3926);
        assert_eq!(crc32(&[b"IEND"]), 0xAE42_6082);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
        assert_eq!(adler32(b""), 1);
    }

    #[test]
    fn the_empty_stream_is_the_known_fixed_block() {
        assert_eq!(deflate(&[]), vec![0x03, 0x00]);
        assert_eq!(deflate(b"a"), vec![0x4B, 0x04, 0x00]);
        assert_eq!(inflate(&[0x03, 0x00]).unwrap(), Vec::<u8>::new());
        assert_eq!(inflate(&[0x4B, 0x04, 0x00]).unwrap(), b"a".to_vec());
    }

    #[test]
    fn deflate_round_trips_runs_literals_and_every_byte_value() {
        let mut samples: Vec<Vec<u8>> = vec![
            vec![7; 1000],
            (0..=255).collect(),
            [1u8, 2, 3, 4].repeat(300),
            noise(5000),
            vec![0; 258 * 3 + 2],
            vec![9, 9],
        ];
        let mut mixed = noise(300);
        mixed.extend(vec![200u8; 400]);
        mixed.extend(noise(10));
        samples.push(mixed);
        for sample in samples {
            let packed = deflate(&sample);
            assert_eq!(inflate(&packed).unwrap(), sample);
        }
        assert!(deflate(&vec![0; 100_000]).len() < 1000);
    }

    #[test]
    fn pictures_round_trip_and_flat_areas_shrink() {
        let (width, height) = (37, 21);
        let mut rgba = noise(width * height * 4);
        for pixel in rgba.chunks_exact_mut(4).skip(width * 5) {
            pixel.copy_from_slice(&[31, 58, 95, 255]);
        }
        let file = encode(width, height, &rgba);
        assert_eq!(&file[..8], &SIGNATURE);
        assert_eq!(&file[12..16], b"IHDR");
        assert_eq!(&file[file.len() - 12..], &[0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82]);
        assert_eq!(decode(&file).unwrap(), (width, height, rgba));
        let flat = encode(256, 256, &[31, 58, 95, 255].repeat(256 * 256));
        assert!(flat.len() < 2000, "{} bytes", flat.len());
        assert_eq!(decode(&encode(0, 0, &[])).unwrap(), (0, 0, Vec::new()));
    }

    #[test]
    fn damaged_pictures_are_rejected() {
        let file = encode(4, 4, &[1; 64]);
        assert!(decode(&file[..20]).is_err());
        assert!(decode(b"not a picture").is_err());
        let mut flipped = file.clone();
        let middle = flipped.len() - 20;
        flipped[middle] ^= 0x40;
        assert!(decode(&flipped).is_err());
        let mut gray = file;
        gray[25] = 0;
        assert!(decode(&gray).is_err());
    }
}
