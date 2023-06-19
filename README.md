# Handlevec

Small abstraction over index-style iteration over a vector, with deletion, insertion, and other operations on the vector while iterating.
Hopefully, less bug-prone and headache-prone than manually considering index updates.

# Simple example:

```rust
use handlevec::mutate_vec_by_handles;

let mut my_vec = vec![1, 4, 9, 16, 25, 36, 49, 64, 81, 100];

mutate_vec_by_handles(&mut my_vec, |mut elem| {
    // Get and copy the next element, if it exists (it must be copied because of the borrow checker.)
    if let Some(n) = elem.peek_forward_slice(1).copied() {
        *elem.get_mut() *= n; // Multiply this element by the next element, in-place.
    } else {
        elem.discard(); // Discard this element if there is no next element
    }
});

assert_eq!(my_vec, vec![4, 36, 144, 400, 900, 1764, 3136, 5184, 8100]);

// Or you can add it as a trait extension, if preferred:
use handlevec::VecMutateByHandles;

my_vec.mutate_vec_by_handles(|mut element| {
    if *element.get() == 900 {
        element.insert_and_skip(50);
    }
});

assert_eq!(my_vec, vec![4, 36, 144, 400, 900, 50, 1764, 3136, 5184, 8100]);
```

# Example with while loop, if you don't like closures:

```rust
use handlevec::VecMutationHandle;

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
```

For most of these examples, it might have been far better to use normal iterators, maybe flatmap or filter or map.
The particular use case in mind here, is if you have a vector of a more complex data-type, where various cases may
require very different processing.

Please note that this package does not in any way attempt to "buffer" changes done to the vector. Changes are applied at function call.
For long vectors and many insertions or deletions, reorganizing the vector after each iteration might not be very performant.

This does contain unwraps, but they are never supposed to be reachable using any inputs on the public API.
If you get a panic from this crate, a bug report is very appreciated.


# Features
1. Loop through a vector using one of the ways above (they are equivalent), and for each element:
2. Get a (potentially mutable) reference to the current element.
3. Set a new value to the current element.
4. Discard the current element (and get ownership of that element in return, but no other operations can be applied to this element.)
5. Insert an element after the current element, and process it in the next iteration.
6. Insert an element after the current element, but do not process it in the next iteration.
7. Skip processing a specific number of elements.
8. Stop iteration. The remainder of the closure or loop is still executed, as the method must return, but no further elements are processed.
9. Discard the current element, and stop the iteration. Both the `discard` and `stop_iteration` methods consume ownership of the handle, so this is provided if you want to do both.
10. "Peek" a (potentially mutable) reference to a slice of the vector, with 0 being the index of the current element. E.g. `1` is the next element, and `0..` is a slice of the remaining elements, including this one.
11. Insert multiple elements, in the correct order. (calling insert multiple times will reverse the order of the inserted elements, akin to a stack push.)
12. Replace the element at a specific place with another one.
13. Finally, the closure is an `FnMut`, so the inner loop can affect mutable variables outside the closure.
By design, mutating or obtaining elements prior to the current one is not allowed.
