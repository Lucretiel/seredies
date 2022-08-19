/*!
Primitive writers for RESP components
*/

use std::{io, slice};

pub fn write_header()

#[inline]
#[must_use]
const fn is_newline_byte(b: u8) -> bool {
    b == b'\n' || b == b'\r'
}

struct NewlineRejector<W> {
    inner: W,
}

fn advance_buffers<'a>(
    buffers: &mut &mut [io::IoSlice<'a>],
    mut amount: usize,
    mut save: Option<io::IoSlice<'a>>,
) -> Option<io::IoSlice<'_>> {
    while let Some((head, tail)) = buffers.split_first_mut() {
        if head.len() <= amount {
            amount -= head.len();

            if let Some(saved) = save.take() {
                *head = saved;
            }

            *buffers = tail;
        } else if amount == 0 {
            return save;
        } else {
            let save = save.unwrap_or(*head);
            *head = io::IoSlice::new(&head[amount..]);
            return Some(save);
        }
    }
}

fn write_all_vectored(
    dest: &mut impl io::Write,
    mut buffers: &mut [io::IoSlice<'_>],
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

impl BufferManager<'a> {
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

    pub fn advance(&mut self, amount: usize) {
        while let Some((head, tail)) = self.buffers.split_first_mut() {
            // The head is smaller than the overall write, so pop it off the
            // the front of the buffers. Be sure to restore the original state,
            // if necessary.
            if head.len() <= amount {
                amount -= head.len();

                if let Some(saved) = self.saved.take() {
                    *head = saved;
                }

                *buffers = tail;
            }
            // The head is larger than the overall write, so it needs to be
            // modified in place (if any bytes were written at all)
            else {
                if amount > 0 {
                    // We're mutating the head. If we don't already have a saved
                    // copy of the original, save it now.
                    self.saved = Some(self.saved.unwrap_or(*head));
                    *head = io::IoSlice::new(&head[amount..]);
                }

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