use std::{borrow::Cow, collections::VecDeque};

pub struct DropBomb {
    msg: Cow<'static, str>,
    defused: bool,
}

impl DropBomb {

    pub fn new(msg: impl Into<Cow<'static, str>>) -> Self {
        DropBomb {
            msg: msg.into(),
            defused: false
        }
    }

    pub fn defuse(&mut self) {
        self.defused = true;
    }

}

impl Drop for DropBomb {
    fn drop(&mut self) {
        if !self.defused && !::std::thread::panicking() {
            panic!("{}", self.msg)
        }
    }
}

pub struct SkipLast<I: Iterator> {
    buffer: VecDeque<I::Item>,
    iter: I,
    count: usize,
}

impl <I: Iterator> Iterator for SkipLast<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        while self.buffer.len() <= self.count {
            match self.iter.next() {
                None => {
                    // More items to drop than to return
                    self.buffer.clear();
                    return None;
                }
                Some(item) => {
                    self.buffer.push_back(item);
                }
            }
        }
        // If we arrived here, always present
        self.buffer.pop_front()
    }
}

pub trait IterExt : Iterator {
    fn skip_last(self, n: usize) -> SkipLast<Self> where Self: Sized {
        SkipLast {
            buffer: VecDeque::new(),
            iter: self,
            count: n,
        }
    }
}

impl<T> IterExt for T where T: Iterator + ?Sized {}

#[cfg(test)]
mod test {

    use crate::util::IterExt;

    #[test]
    fn test_skip_last_two() {
        let v = vec![1,2,3,4,5];
        let w: Vec<_> = v.iter().skip_last(2).collect();
        assert_eq!(w.len(), 3);
        assert_eq!(*w[0], 1);
        assert_eq!(*w[1], 2);
        assert_eq!(*w[2], 3);
    }

    #[test]
    fn test_skip_nothing() {
        let v = vec![1,2,3,4,5];
        let w: Vec<_> = v.iter().skip_last(0).collect();
        assert_eq!(w.len(), 5);
        assert_eq!(*w[0], 1);
        assert_eq!(*w[1], 2);
        assert_eq!(*w[2], 3);
        assert_eq!(*w[3], 4);
        assert_eq!(*w[4], 5);
    }

}
