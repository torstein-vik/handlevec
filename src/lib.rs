#![deny(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![warn(clippy::cargo)]

//! Small abstraction over index-style iteration over a vector, with deletion, insertion, and other operations on the vector while iterating.
//! Hopefully, less bug-prone and headache-prone than manually considering index updates.
//!
//! # Simple example:
//! ```
//! use handlevec::mutate_vec_by_handles;
//! let mut my_vec = vec![1, 4, 9, 16, 25, 36, 49, 64, 81, 100];
//!
//! mutate_vec_by_handles(&mut my_vec, |mut elem| {
//!     // Get and copy the next element, if it exists (it must be copied because of the borrow checker.)
//!     if let Some(n) = elem.peek_forward_slice(1).copied() {
//!         *elem.get_mut() *= n; // Multiply this element by the next element, in-place.
//!     } else {
//!         elem.discard(); // Discard this element if there is no next element
//!     }
//! });
//!
//! assert_eq!(my_vec, vec![4, 36, 144, 400, 900, 1764, 3136, 5184, 8100]);
//!
//! // Or you can add it as a trait extension, if preferred:
//! use handlevec::VecMutateByHandles;
//!
//! my_vec.mutate_vec_by_handles(|mut element| {
//!     if *element.get() == 900 {
//!         element.insert_and_skip(50);
//!     }
//! });
//!
//! assert_eq!(my_vec, vec![4, 36, 144, 400, 900, 50, 1764, 3136, 5184, 8100]);
//! ```
//!
//! # Example with while loop, if you don't like closures:
//! ```
//! use handlevec::VecMutationHandle;
//! let mut my_vec = vec![2, 3, 4, 5, 6, 11, 1, 5, 7];
//!
//! let mut my_index = 0;
//!
//! while let Some(mut elem) = VecMutationHandle::new(&mut my_vec, &mut my_index) {
//!     if *elem.get() > 10 {
//!        elem.discard_and_stop_iteration();
//!     } else {
//!        elem.set(20);
//!     }
//! }
//!
//! assert_eq!(my_vec, vec![20, 20, 20, 20, 20, 1, 5, 7]);
//! ```
//!
//! For most of these examples, it might have been far better to use normal iterators, maybe flatmap or filter or map.
//! The particular use case in mind here, is if you have a vector of a more complex data-type, where various cases may
//! require very different processing.
//!
//! Please note that this package does not in any way attempt to "buffer" changes done to the vector. Changes are applied at function call.
//! For long vectors and many insertions or deletions, reorganizing the vector after each iteration might not be very performant.
//!
//! This does contain unwraps, but they are never supposed to be reachable using any inputs on the public API.
//! If you get a panic from this crate, a bug report is very appreciated.
//!
//! # Features
//! 1. Loop through a vector using one of the ways above (they are equivalent), and for each element:
//! 2. Get a (potentially mutable) reference to the current element.
//! 3. Set a new value to the current element.
//! 4. Discard the current element (and get ownership of that element in return, but no other operations can be applied to this element.)
//! 5. Insert an element after the current element, and process it in the next iteration.
//! 6. Insert an element after the current element, but do not process it in the next iteration.
//! 7. Skip processing a specific number of elements.
//! 8. Stop iteration. The remainder of the closure or loop is still executed, as the method must return, but no further elements are processed.
//! 9. Discard the current element, and stop the iteration. Both the `discard` and `stop_iteration` methods consume ownership of the handle, so this is provided if you want to do both.
//! 10. "Peek" a (potentially mutable) reference to a slice of the vector, with 0 being the index of the current element. E.g. `1` is the next element, and `0..` is a slice of the remaining elements, including this one.
//! 11. Insert multiple elements, in the correct order. (calling insert multiple times will reverse the order of the inserted elements, akin to a stack push.)
//! 12. Replace the element at a specific place with another one.
//! 13. Finally, the closure is an `FnMut`, so the inner loop can affect mutable variables outside the closure.
//!
//! By design, mutating or obtaining elements prior to the current one is not allowed.

pub use crate::vec_mut_handle_core::*;

// Core of vector mutations. Attempt to keep small, to have guaranteed no panics. Sealed in it's own module to restrict surface area.
mod vec_mut_handle_core {
    use std::slice::SliceIndex;

    // Contract:
    // `index < vec.len()`
    // `next_index >= index`
    // `vec` may not be mutated at indices smaller than `index`
    // as long as all internal methods respect and preserve these, all of them may assume these.
    /// Represents an index in a vector, allowing mutation of the vector with that index as a "context".
    #[derive(Debug)]
    pub struct VecMutationHandle<'a, 'b, T> {
        vec: &'a mut Vec<T>,
        index: usize,              // The current index. Should not be mutated.
        next_index: &'b mut usize, // The index for the next iteration. Mutated e.g. when element is removed, so none are skipped.
    }

    impl<'a, 'b, T> VecMutationHandle<'a, 'b, T> {
        /// Creates a vector mutation handle, allowing mutation of a vector with a specific element (index) as a "context".
        /// Mutates this index reference, so that it points to the next element in the vector that should be processed.
        ///
        /// Provides `None` if index is less than vector length (iteration should be stopped).
        /// In case the `index` is valid, `index` is always immediately overwritten with `index + 1`
        /// (and a copy of the original value is used inside here), even if no methods are called on the handle.
        /// Future methods may alter this index further. It may contain "junk" values like `usize::MAX` afterwards (in the case of stopping iteration).
        /// Ideally, nothing other than this crate should depend on the value of the index reference.
        #[must_use]
        pub fn new(vec: &'a mut Vec<T>, index: &'b mut usize) -> Option<Self> {
            let curr_index: usize = *index;
            if curr_index < vec.len() {
                *index = curr_index + 1;
                Some(VecMutationHandle {
                    vec,
                    index: curr_index,
                    next_index: index,
                })
            } else {
                None
            }
        }

        /// Get a reference to the current element.
        /// # Panics
        /// Might panic in case of a bug in this crate, due to a potentially invalid index.
        #[must_use]
        pub fn get(&self) -> &T {
            self.vec.get(self.index).unwrap() // From the new method, we are always within bounds. The discard method consumes ownership. This is ok.
        }

        /// Get a mutable reference to the current element.
        /// # Panics
        /// Might panic in case of a bug in this crate, due to a potentially invalid index.
        #[must_use]
        pub fn get_mut(&mut self) -> &mut T {
            self.vec.get_mut(self.index).unwrap() // From the new method, we are always within bounds. The discard method consumes ownership. This is ok.
        }

        #[allow(clippy::must_use_candidate)]
        /// Remove the current element, and return it as owned.
        /// Consumes self, as the contract is now invalid (index could be larger than or equal to vec length, especially if we repeat discarding.)
        pub fn discard(self) -> T {
            *self.next_index -= 1;
            self.vec.remove(self.index)
        }

        /// Insert a new element AFTER the current one, and process it in the next iteration (specifically, do not shift the index to ignore this element).
        pub fn insert_and_process(&mut self, t: T) {
            // This looks weird, accessing index + 1. But insert allows the length as an index, in that case inserting after all other elements.
            self.vec.insert(self.index + 1, t);
        }

        /// Skip a certain amount of the next elements.
        pub fn skip_forward(&mut self, steps_to_skip: usize) {
            *self.next_index += steps_to_skip;
        }

        /// Do not process any more elements (equivalent to `skip_forward` more elements than remain in the vector)
        /// Please note, this does not affect the call-site like the `break` keyword. This method does return, and executation continues from the call-site.
        /// The index reference is set to `usize::MAX` to achieve this.
        pub fn stop_iteration(self) {
            *self.next_index = usize::MAX; // If your vector is larger than usize::MAX, then you have another problem anyway...
        }

        /// Discards the current element, and returns it as owned. Does not process any more elements.
        /// Both the `discard` and `stop_iteration` methods consume ownership of the handle, so this is provided if you want to do both.
        #[allow(clippy::must_use_candidate)]
        pub fn discard_and_stop_iteration(self) -> T {
            *self.next_index = usize::MAX;
            self.vec.remove(self.index)
        }

        /// "Peek" a reference to a slice of the vector, with 0 being the index of the current element. E.g. `1` is the next element, and `0..` is a slice of the remaining elements, including this one.
        #[must_use]
        pub fn peek_forward_slice<I>(&self, slice: I) -> Option<&I::Output>
        where
            I: SliceIndex<[T]>,
        {
            self.vec.get(self.index..)?.get(slice)
        }

        /// "Peek" a mutable reference to a slice of the vector, with 0 being the index of the current element. E.g. `1` is the next element, and `0..` is a slice of the remaining elements, including this one.
        #[must_use]
        pub fn peek_forward_slice_mut<I>(&mut self, slice: I) -> Option<&mut I::Output>
        where
            I: SliceIndex<[T]>,
        {
            self.vec.get_mut(self.index..)?.get_mut(slice)
        }
    }
}

impl<'a, 'b, T> VecMutationHandle<'a, 'b, T> {
    /// Insert a new element AFTER the current one, but do not process it in the next iteration (specifically, shift the index as to ignore this element).
    pub fn insert_and_skip(&mut self, t: T) {
        self.insert_and_process(t);
        self.skip_forward(1);
    }

    /// Assign a new value to this element.
    pub fn set(&mut self, t: T) {
        *self.get_mut() = t;
    }

    /// Replace the current element with another, and get ownership of the value currently there.
    pub fn replace(&mut self, t: T) -> T {
        let curr = self.get_mut();
        std::mem::replace(curr, t)
    }

    /// Insert each element in a vec, ordering the elements with the same order as the vec. Process the vector elements afterwards.
    pub fn insert_and_process_vec(&mut self, vec: Vec<T>) {
        // Reversed for preserving correct order
        for t in vec.into_iter().rev() {
            self.insert_and_process(t);
        }
    }

    /// Insert each element in a vec, ordering the elements with the same order as the vec. Do not process the vector elements afterwards.
    pub fn insert_and_skip_vec(&mut self, vec: Vec<T>) {
        let steps_to_skip = vec.len();
        self.insert_and_process_vec(vec);
        self.skip_forward(steps_to_skip);
    }
}

/// Mutate a vec using index-style looping, but without thinking about the indices.
///
/// See crate documentation for examples and more context.
pub fn mutate_vec_by_handles<T>(vec: &mut Vec<T>, mut op: impl FnMut(VecMutationHandle<T>)) {
    let mut curr_index = 0;

    while let Some(handle) = VecMutationHandle::new(vec, &mut curr_index) {
        op(handle);
    }
}

/// Trait for adding vector mutation by handles as an extension trait to vec.
pub trait VecMutateByHandles<T>: Sized {
    /// Mutate a vec using index-style looping, but without thinking about the indices.
    ///
    /// See crate documentation for examples and more context.
    fn mutate_vec_by_handles(&mut self, op: impl FnMut(VecMutationHandle<T>));
}

impl<T> VecMutateByHandles<T> for Vec<T> {
    fn mutate_vec_by_handles(&mut self, op: impl FnMut(VecMutationHandle<T>)) {
        mutate_vec_by_handles(self, op);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vec_mut_handle_new() {
        let mut v = vec![1, 2, 3];
        let mut index = 0;
        let handle = VecMutationHandle::new(&mut v, &mut index);
        assert!(handle.is_some());
        assert_eq!(handle.unwrap().get(), &1);
    }

    #[test]
    fn test_vec_mut_handle_set() {
        let mut v = vec![1, 2, 3];
        let mut index = 0;
        let mut handle = VecMutationHandle::new(&mut v, &mut index).unwrap();
        handle.set(10);
        assert_eq!(handle.get(), &10);
    }

    #[test]
    fn test_vec_mut_handle_discard() {
        let mut v = vec![1, 2, 3];
        let mut index = 0;
        let handle = VecMutationHandle::new(&mut v, &mut index).unwrap();
        assert_eq!(handle.discard(), 1);
        assert_eq!(v, vec![2, 3]);
    }

    #[test]
    fn test_vec_mut_handle_insert_and_process() {
        let mut v = vec![1, 2, 3];
        let mut index = 0;
        let mut handle = VecMutationHandle::new(&mut v, &mut index).unwrap();
        handle.insert_and_process(10);
        assert_eq!(v, vec![1, 10, 2, 3]);
        assert_eq!(index, 1);
    }

    #[test]
    fn test_vec_mut_handle_skip_forward() {
        let mut v = vec![1, 2, 3];
        let mut index = 0;
        let mut handle = VecMutationHandle::new(&mut v, &mut index).unwrap();
        handle.skip_forward(2);
        assert_eq!(index, 3);
    }

    #[test]
    fn test_vec_mut_handle_peek_forward_slice() {
        let mut v = vec![1, 2, 3];
        let mut index = 0;
        let handle = VecMutationHandle::new(&mut v, &mut index).unwrap();
        assert_eq!(handle.peek_forward_slice(1..), Some(&[2, 3][..]));
        let handle_two = VecMutationHandle::new(&mut v, &mut index).unwrap();
        assert_eq!(handle_two.peek_forward_slice(1..), Some(&[3][..]));
        assert_eq!(handle_two.peek_forward_slice(2), None);
    }

    #[test]
    fn test_vec_mut_handle_peek_forward_slice_mut() {
        let mut v = vec![1, 2, 3];
        let mut index = 0;
        let mut handle = VecMutationHandle::new(&mut v, &mut index).unwrap();
        assert_eq!(handle.peek_forward_slice_mut(1..), Some(&mut [2, 3][..]));
        handle.peek_forward_slice_mut(1..).unwrap()[1] = 70;
        assert_eq!(v[2], 70);
    }

    #[test]
    fn test_vec_mut_handle_insert_and_skip() {
        let mut v = vec![1, 2, 3];
        let mut index = 1;
        let mut handle = VecMutationHandle::new(&mut v, &mut index).unwrap();
        handle.insert_and_skip(10);
        assert_eq!(v, vec![1, 2, 10, 3]);
        assert_eq!(index, 3);
    }

    #[test]
    fn test_mutate_vec_mutate_vec_set() {
        let mut v = vec![1, 2, 3];
        v.mutate_vec_by_handles(|mut handle| {
            handle.set(10);
        });
        assert_eq!(v, vec![10, 10, 10]);
    }

    #[test]
    fn test_mutate_vec_mutate_vec_insert() {
        let mut v = vec![1, 2, 3];
        v.mutate_vec_by_handles(|mut handle| {
            handle.insert_and_skip(10);
        });
        assert_eq!(v, vec![1, 10, 2, 10, 3, 10]);
    }

    #[test]
    fn test_mutate_vec_complex_insertion_and_deletion() {
        let mut v = vec![1, 2, 3, 4, 5];
        v.mutate_vec_by_handles(|mut handle| {
            if *handle.get() == 3 {
                handle.insert_and_skip(100);
                handle.discard();
            }
        });
        assert_eq!(v, vec![1, 2, 100, 4, 5]);
    }

    #[test]
    fn test_mutate_vec_complex_insertion_and_modification() {
        let mut v = vec![1, 2, 3, 4, 5];
        v.mutate_vec_by_handles(|mut handle| {
            if *handle.get() == 3 {
                handle.insert_and_skip(100);
                handle.set(50);
            }
        });
        assert_eq!(v, vec![1, 2, 50, 100, 4, 5]);
    }

    #[test]
    fn test_mutate_vec_complex_peeks_and_state_insert_process() {
        let mut my_vec = vec![2, 3, 4, 5, 6, 7, 1];
        let mut my_count = 0;

        my_vec.mutate_vec_by_handles(|mut elem| {
            let val = *elem.get();
            my_count += val;

            if val > 6 {
                elem.discard();
            } else if val < 3 {
                let x = elem.peek_forward_slice(0..).unwrap().len();
                elem.set(x);
            } else if val == 4 {
                elem.insert_and_process(7);
            }
        });

        assert_eq!(my_count, 35);
        assert_eq!(my_vec, vec![7, 3, 4, 5, 6, 1]);
    }

    #[test]
    fn test_vec_example() {
        let mut my_vecs = vec![
            vec![1, 2, 3, 4, 5],
            vec![1, 2, 3],
            vec![1, 2, 3, 13],
            vec![5],
        ];

        let mut deleted_vecs = vec![];

        my_vecs.mutate_vec_by_handles(|mut handle| {
            if handle.get().len() != 5 {
                handle.get_mut().resize(5, 0);
            }

            if handle.get().contains(&13) {
                let deleted_vec = handle.discard();
                deleted_vecs.push(deleted_vec);
            }
        });

        assert_eq!(deleted_vecs, vec![vec![1, 2, 3, 13, 0]]);
        assert_eq!(
            my_vecs,
            vec![
                vec![1, 2, 3, 4, 5],
                vec![1, 2, 3, 0, 0],
                vec![5, 0, 0, 0, 0],
            ]
        );
    }

    #[test]
    fn test_mutate_vec_complex_break_loop() {
        let mut my_vec = vec![2, 3, 4, 5, 6, 11, 1, 5, 7];

        my_vec.mutate_vec_by_handles(|elem| {
            if *elem.get() > 10 {
                elem.stop_iteration();
            } else {
                elem.discard();
            }
        });

        assert_eq!(my_vec, vec![11, 1, 5, 7]);
    }

    #[test]
    fn test_mutate_vec_function_complex_break_loop() {
        let mut my_vec = vec![2, 3, 4, 5, 6, 11, 1, 5, 7];

        mutate_vec_by_handles(&mut my_vec, |elem| {
            if *elem.get() > 10 {
                elem.stop_iteration();
            } else {
                elem.discard();
            }
        });

        assert_eq!(my_vec, vec![11, 1, 5, 7]);
    }

    #[test]
    fn test_mutate_vec_peek() {
        let mut my_vec = vec![1, 4, 9, 16, 25, 36, 49, 64, 81, 100];

        mutate_vec_by_handles(&mut my_vec, |mut elem| {
            // Get and copy the next element, if it exists
            if let Some(n) = elem.peek_forward_slice(1).copied() {
                *elem.get_mut() *= n; // Multiply this element by the next element, in-place.
            } else {
                elem.discard(); // Discard this element if there is no next element
            }
        });

        assert_eq!(my_vec, vec![4, 36, 144, 400, 900, 1764, 3136, 5184, 8100]);
    }

    #[test]
    fn test_mutate_vec_swap() {
        let mut my_vec = vec![1, 4, 9, 16, 25, 36, 49, 64, 81, 100];

        let mut x = 1;

        mutate_vec_by_handles(&mut my_vec, |mut elem| {
            x = elem.replace(x);
        });

        assert_eq!(my_vec, vec![1, 1, 4, 9, 16, 25, 36, 49, 64, 81]);
        assert_eq!(x, 100);
    }

    #[test]
    fn test_mutate_vec_handrolled_complex_break_loop() {
        let mut my_vec = vec![2, 3, 4, 5, 6, 11, 1, 5, 7];

        let mut my_index = 0;

        while let Some(elem) = VecMutationHandle::new(&mut my_vec, &mut my_index) {
            if *elem.get() > 10 {
                elem.stop_iteration();
            } else {
                elem.discard();
            }
        }

        assert_eq!(my_vec, vec![11, 1, 5, 7]);
    }

    #[test]
    fn test_mutate_vec_handrolled_complex_break_loop_with_final_discard() {
        let mut my_vec = vec![2, 3, 4, 5, 6, 11, 1, 5, 7];

        let mut my_index = 0;

        while let Some(mut elem) = VecMutationHandle::new(&mut my_vec, &mut my_index) {
            if *elem.get() > 10 {
                elem.discard_and_stop_iteration();
            } else {
                elem.set(20);
            }
        }

        assert_eq!(my_vec, vec![20, 20, 20, 20, 20, 1, 5, 7]);
    }
}
