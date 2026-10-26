use crate::Result;
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct Tokenizer {
    pieces: Vec<Vec<u8>>,
    scores: Vec<f64>,
    lookup: BTreeMap<Vec<u8>, usize>,
}
impl Tokenizer {
    pub fn parse(bytes: &[u8], vocab: usize) -> Result<Self> {
        if bytes.len() < 4 || bytes.len() > 1 << 20 || !(259..=1024).contains(&vocab) {
            return Err("tokenizer byte/vocabulary limit".into());
        }
        let maximum = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
        if maximum == 0 || maximum > 128 {
            return Err("tokenizer piece limit".into());
        }
        let mut offset = 4;
        let mut pieces = Vec::new();
        let mut scores = Vec::new();
        let mut lookup = BTreeMap::new();
        for token in 0..vocab {
            if offset + 8 > bytes.len() {
                return Err("truncated tokenizer".into());
            }
            let score = f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as f64;
            let len =
                u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
            offset += 8;
            if !score.is_finite() || len == 0 || len > maximum || offset + len > bytes.len() {
                return Err("invalid tokenizer score/piece".into());
            }
            let piece = bytes[offset..offset + len].to_vec();
            offset += len;
            std::str::from_utf8(&piece).map_err(|_| "tokenizer piece is not UTF-8")?;
            if lookup.insert(piece.clone(), token).is_some() {
                return Err("duplicate tokenizer piece".into());
            }
            pieces.push(piece);
            scores.push(score);
        }
        if offset != bytes.len()
            || pieces[0] != b"<unk>"
            || pieces[1] != b"\n<s>\n"
            || pieces[2] != b"\n</s>\n"
            || !lookup.contains_key(&b" "[..])
        {
            return Err("unsupported tokenizer inventory or trailing data".into());
        }
        for b in 0..256 {
            if pieces[b + 3] != format!("<0x{b:02X}>").as_bytes() {
                return Err("unsupported tokenizer byte fallback table".into());
            }
        }
        Ok(Self {
            pieces,
            scores,
            lookup,
        })
    }
    pub fn encode(&self, text: &str, bos: bool, eos: bool) -> Result<Vec<usize>> {
        if text.len() > 4096 || text.contains('\0') {
            return Err("tokenizer text byte limit or embedded NUL".into());
        }
        let mut tokens = Vec::new();
        if bos {
            tokens.push(1);
        }
        if !text.is_empty() {
            tokens.push(self.lookup[&b" "[..]]);
        }
        for ch in text.chars() {
            let mut buffer = [0; 4];
            let bytes = ch.encode_utf8(&mut buffer).as_bytes();
            if let Some(id) = self.lookup.get(bytes) {
                tokens.push(*id);
            } else {
                tokens.extend(bytes.iter().map(|b| *b as usize + 3));
            }
        }
        loop {
            let mut best = None;
            let mut score = -1e10;
            for (index, pair) in tokens.windows(2).enumerate() {
                let joined = [
                    self.pieces[pair[0]].as_slice(),
                    self.pieces[pair[1]].as_slice(),
                ]
                .concat();
                if let Some(id) = self.lookup.get(&joined) {
                    if self.scores[*id] > score {
                        score = self.scores[*id];
                        best = Some((index, *id));
                    }
                }
            }
            if let Some((index, id)) = best {
                tokens[index] = id;
                tokens.remove(index + 1);
            } else {
                break;
            }
        }
        if eos {
            tokens.push(2);
        }
        Ok(tokens)
    }
    pub fn decode_bytes(&self, tokens: &[usize]) -> Result<Vec<u8>> {
        if tokens.len() > 4098 {
            return Err("decode token limit".into());
        }
        let mut out = Vec::new();
        let mut previous = 1;
        for token in tokens {
            let piece = self
                .pieces
                .get(*token)
                .ok_or("token ID outside vocabulary")?;
            if *token != 1 && *token != 2 {
                if (3..259).contains(token) {
                    out.push((token - 3) as u8);
                } else if previous == 1 && piece.first() == Some(&b' ') {
                    out.extend_from_slice(&piece[1..]);
                } else {
                    out.extend_from_slice(piece);
                }
            }
            previous = *token;
        }
        Ok(out)
    }
    pub fn decode(&self, tokens: &[usize]) -> Result<String> {
        String::from_utf8(self.decode_bytes(tokens)?)
            .map_err(|_| "decoded byte stream is incomplete/invalid UTF-8".into())
    }
}
