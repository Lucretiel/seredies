/*!
Primitive writers for RESP components
*/

use std::io;

#[inline]
#[must_use]
const fn is_newline_byte(b: u8) -> bool {
    b == b'\n' || b == b'\r'
}

struct NewlineRejector<W> {
    inner: W,
}

pub fn write_all_vectored<'a>(
    dest: &mut impl io::Write,
    mut buffers: &'a mut [io::IoSlice<'a>],
) -> io::Result<()> {
    let mut buffers = BufferManager::new(buffers);

    while buffers.count() > 0 {
        match dest.write_vectored(buffers.get()) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(n) => buffers.advance(n),
            Err(err) if err.kind() == io::ErrorKind::Interrupted => {}
            Err(err) => return Err(err),
        }
    }

    Ok(())
}

struct BufferManager<'a> {
    buffers: &'a mut [io::IoSlice<'a>],
    saved: Option<io::IoSlice<'a>>,
}

impl<'a> BufferManager<'a> {
    pub fn new(buffers: &'a mut [io::IoSlice<'a>]) -> Self {
        // strip empty buffers
        let first_non_empty = buffers
            .iter()
            .position(|buf| buf.len() > 0)
            .unwrap_or(buffers.len());

        let buffers = &mut buffers[first_non_empty..];

        Self {
            buffers,
            saved: None,
        }
    }

    pub fn get(&self) -> &[io::IoSlice<'a>] {
        self.buffers
    }

    pub fn count(&self) -> usize {
        self.buffers.len()
    }

    pub fn advance(&mut self, mut amount: usize) {
        while let Some((head, tail)) = self.buffers.split_first_mut() {
            if head.len() <= amount {
                amount -= head.len();

                if let Some(saved) = self.saved.take() {
                    *head = saved;
                }

                self.buffers = tail;
            }
            // The head is larger than the overall write, so it needs to be
            // modified in place (if any bytes were written at all)
            else {
                if self.saved.is_none() {
                    self.saved = Some(*head);
                }

                *head = io::IoSlice::new(&head.ge[amount..]);

                return;
            }
        }

        assert!(amount == 0, "advanced too far")
    }
}

impl Drop for BufferManager<'_> {
    fn drop(&mut self) {
        // When the buffer manager is dropped, restore the state of the
        // current buffers head, if necessary. It shouldn't be possible for
        // there to be a saved value while the buffer list is empty.
        if let Some(head) = self.buffers.first_mut() {
            if let Some(saved) = self.saved {
                *head = saved
            }
        }
    }
}
