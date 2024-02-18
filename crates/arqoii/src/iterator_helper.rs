use core::{iter::FusedIterator, mem::MaybeUninit};

pub(crate) struct PeekN<const N: usize, I, Item> {
    iter: I,
    peek: [Option<Item>; N],
}

impl<const N: usize, I, Item> PeekN<N, I, Item> {
    pub fn new(iter: I) -> Self {
        Self {
            iter,
            peek: [(); N].map(|_| None),
        }
    }

    /// peek at the next N items if at least N more elements are available
    pub fn peek(&mut self) -> Option<[&Item; N]>
    where
        I: Iterator<Item = Item>,
    {
        // rotate the first remaining peek value to the front
        let rotate = self
            .peek
            .iter()
            .enumerate()
            .find_map(|(idx, elem)| elem.is_some().then_some(idx))
            .unwrap_or(0);
        self.peek.rotate_left(rotate);

        let mut peek = [(); N].map(|_| MaybeUninit::uninit());
        let mut count = 0;

        for elem in self.peek.iter_mut().flat_map(|elem| {
            if elem.is_none() {
                *elem = self.iter.next();
            }
            elem.as_ref()
        }) {
            peek[count].write(elem);
            count += 1;
        }

        if count == N {
            // Safety count is N and as such all indices 0 to N - 1 have been written to
            Some(unsafe { core::mem::transmute_copy::<[MaybeUninit<&Item>; N], [&Item; N]>(&peek) })

            // once https://github.com/rust-lang/rust/issues/96097 is stabilized use
            // Some(unsafe { peek.transpose().assume_init()})
            // or
            // Some(unsafe { MaybeUninit::array_assume_init(peek) })
        } else {
            None
        }
    }
}

impl<const N: usize, I: Iterator> Iterator for PeekN<N, I, I::Item> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.peek
            .iter_mut()
            .find_map(|elem| elem.take())
            .or_else(|| self.iter.next())
    }
}

impl<const N: usize, I, Item> FusedIterator for PeekN<N, I, Item>
where
    I: FusedIterator,
    PeekN<N, I, Item>: Iterator,
{
}
