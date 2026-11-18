//! Several shells on one screen. Each leaf is one session.

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum DropSide {
    Left,
    Right,
    Above,
    Below,
}

#[derive(Clone)]
pub(super) enum Screens {
    Shell(usize),
    Split {
        id: u64,
        side_by_side: bool,
        ratio: f32,
        first: Box<Screens>,
        second: Box<Screens>,
    },
}

impl Screens {
    pub(super) fn balanced(ids: &[usize], next_id: &mut u64) -> Self {
        if ids.len() <= 1 {
            return Self::Shell(ids.first().copied().unwrap_or(1));
        }
        let mid = ids.len() / 2;
        let id = *next_id;
        *next_id += 1;
        Self::Split {
            id,
            side_by_side: true,
            ratio: mid as f32 / ids.len() as f32,
            first: Box::new(Self::balanced(&ids[..mid], next_id)),
            second: Box::new(Self::balanced(&ids[mid..], next_id)),
        }
    }

    pub(super) fn contains(&self, shell: usize) -> bool {
        match self {
            Self::Shell(id) => *id == shell,
            Self::Split { first, second, .. } => first.contains(shell) || second.contains(shell),
        }
    }

    pub(super) fn first_leaf(&self) -> Option<usize> {
        match self {
            Self::Shell(id) => Some(*id),
            Self::Split { first, .. } => first.first_leaf(),
        }
    }

    pub(super) fn split_around(
        &mut self,
        current: usize,
        new_shell: usize,
        side_by_side: bool,
        split_id: u64,
    ) -> bool {
        self.place(current, new_shell, side_by_side, true, split_id)
    }

    /// Move an existing screen beside another one.
    pub(super) fn move_beside(&mut self, shell: usize, target: usize, side: DropSide, split_id: u64) -> bool {
        if shell == target || !self.contains(shell) || !self.contains(target) {
            return false;
        }
        self.remove_shell(shell);
        let side_by_side = matches!(side, DropSide::Left | DropSide::Right);
        let after = matches!(side, DropSide::Right | DropSide::Below);
        self.place(target, shell, side_by_side, after, split_id)
    }

    fn place(
        &mut self,
        current: usize,
        incoming: usize,
        side_by_side: bool,
        after: bool,
        split_id: u64,
    ) -> bool {
        match self {
            Self::Shell(id) if *id == current => {
                let (first, second) = if after {
                    (Self::Shell(current), Self::Shell(incoming))
                } else {
                    (Self::Shell(incoming), Self::Shell(current))
                };
                *self = Self::Split {
                    id: split_id,
                    side_by_side,
                    ratio: 0.5,
                    first: Box::new(first),
                    second: Box::new(second),
                };
                true
            }
            Self::Split { first, second, .. } => {
                first.place(current, incoming, side_by_side, after, split_id)
                    || second.place(current, incoming, side_by_side, after, split_id)
            }
            Self::Shell(_) => false,
        }
    }

    pub(super) fn remove_shell(&mut self, shell: usize) {
        if let Some(next) = self.clone().without(shell) {
            *self = next;
        }
    }

    pub(super) fn set_ratio(&mut self, split: u64, ratio: f32) {
        if let Self::Split {
            id,
            ratio: slot,
            first,
            second,
            ..
        } = self
        {
            if *id == split {
                *slot = ratio;
            } else {
                first.set_ratio(split, ratio);
                second.set_ratio(split, ratio);
            }
        }
    }

    fn without(self, shell: usize) -> Option<Self> {
        match self {
            Self::Shell(id) if id == shell => None,
            Self::Shell(id) => Some(Self::Shell(id)),
            Self::Split {
                id,
                side_by_side,
                ratio,
                first,
                second,
            } => match (first.without(shell), second.without(shell)) {
                (None, None) => None,
                (Some(only), None) | (None, Some(only)) => Some(only),
                (Some(first), Some(second)) => Some(Self::Split {
                    id,
                    side_by_side,
                    ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
            },
        }
    }
}
