use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum State {
    #[default]
    Ground,
    Escape,
    Osc,
    Csi,
}

/// What the scanner finds: an OSC's body with the sequence's length from OSC through ST, a full reset, or the title stack's push and pop.
pub(crate) enum Found<'a> {
    Osc(&'a [u8], usize),
    /// An OSC past its bound: the body kept before it.
    Long(&'a [u8]),
    Reset,
    Push,
    Pop,
}

/// Finds each OSC body across reads, ended as vte ends it: at BEL, ESC, CAN or SUB, each full reset, and CSI 22 t and 23 t.
#[derive(Default)]
pub(crate) struct Osc {
    state: State,
    body: Vec<u8>,
    /// The sequence's bytes so far, the controls the body leaves out too.
    length: usize,
    overflow: bool,
    /// A CSI's first parameter while it reads as a plain number, and its fields so far.
    first: Option<u32>,
    fields: usize,
}

impl Osc {
    pub fn feed(&mut self, bytes: &[u8], found: &mut impl FnMut(Found)) {
        let mut rest = bytes;
        while !rest.is_empty() {
            if self.state == State::Ground {
                match memchr::memchr(0x1b, rest) {
                    Some(at) => {
                        self.state = State::Escape;
                        rest = &rest[at + 1..];
                    }
                    None => return,
                }
                continue;
            }
            let byte = rest[0];
            rest = &rest[1..];
            self.state = match (self.state, byte) {
                (State::Escape, b']') => {
                    self.body.clear();
                    self.length = 2;
                    self.overflow = false;
                    State::Osc
                }
                (State::Escape, b'c') => {
                    found(Found::Reset);
                    State::Ground
                }
                (State::Escape, b'[') => {
                    self.first = Some(0);
                    self.fields = 1;
                    State::Csi
                }
                // vte runs controls in an escape, passes DEL and high bytes by, and stays there.
                (State::Escape, 0x00..=0x17 | 0x19 | 0x1b..=0x1f | 0x7f..=0xff) => State::Escape,
                (State::Escape, _) => State::Ground,
                (State::Osc, 0x07 | 0x18 | 0x1a) => {
                    self.finish(1, found);
                    State::Ground
                }
                // ESC ends the body, then reads as an escape: `\` makes ST, counted so.
                (State::Osc, 0x1b) => {
                    self.finish(2, found);
                    State::Escape
                }
                (State::Osc, 0x00..=0x06 | 0x08..=0x17 | 0x19 | 0x1c..=0x1f)
                    if !self.body.starts_with(b"5522;") =>
                {
                    self.length += 1;
                    State::Osc
                }
                (State::Osc, byte) => {
                    self.length += 1;
                    if self.body.len() < self.most() {
                        self.body.push(byte);
                    } else {
                        self.overflow = true;
                    }
                    State::Osc
                }
                // In a CSI, as in vte: CAN and SUB cancel, ESC restarts, controls run, DEL and high bytes pass by.
                (State::Csi, 0x18 | 0x1a) => State::Ground,
                (State::Csi, 0x1b) => State::Escape,
                (State::Csi, 0x00..=0x1f | 0x7f..=0xff) => State::Csi,
                (State::Csi, b'0'..=b'9') => {
                    if self.fields == 1 {
                        self.first = self.first.map(|first| {
                            first
                                .saturating_mul(10)
                                .saturating_add(u32::from(byte - b'0'))
                        });
                    }
                    State::Csi
                }
                (State::Csi, b';' | b':') => {
                    self.fields += 1;
                    State::Csi
                }
                (State::Csi, b't') => {
                    match self.first.filter(|_| self.fields <= PARAMS) {
                        Some(22) => found(Found::Push),
                        Some(23) => found(Found::Pop),
                        _ => {}
                    }
                    State::Ground
                }
                (State::Csi, 0x40..=0x7e) => State::Ground,
                // A private marker or an intermediate makes another sequence.
                (State::Csi, _) => {
                    self.first = None;
                    State::Csi
                }
                (State::Ground, _) => unreachable!("ground scans by position"),
            };
        }
    }

    /// The longest body kept: an OSC 99's holds its payload past its metadata, a title's more.
    fn most(&self) -> usize {
        if self.body.starts_with(b"99;") {
            notify::BODY
        } else if self.body.starts_with(b"5522;") {
            clipboard::BODY
        } else if self.body.starts_with(b"0;") || self.body.starts_with(b"2;") {
            titles::LONGEST
        } else {
            MOST
        }
    }

    fn finish(&mut self, end: usize, found: &mut impl FnMut(Found)) {
        if self.overflow {
            log::warn!("pty: an OSC over {} bytes dropped", self.most());
            found(Found::Long(&self.body));
        } else {
            found(Found::Osc(&self.body, self.length + end));
        }
        self.body.clear();
    }
}

/// The most values, subparameters too, one sequence holds before vte drops it.
pub(super) const PARAMS: usize = 32;
