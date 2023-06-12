use handlevec::VecMutateByHandles;

fn main() {
    let mut my_vec = vec![2, 3, 4, 5, 6, 7, 1];

    let mut my_count = 0;

    println!("{:?}", my_vec);

    my_vec.mutate_vec_by_handles(|mut elem| {
        let val = *elem.get();
        my_count += val;

        if val > 6 {
            elem.discard();
        } else if val < 3 {
            let x = elem.peek_forward_slice(0..).unwrap().len();
            elem.set(x);
        }
    });

    println!("{}", my_count);
    println!("{:?}", my_vec);
}
