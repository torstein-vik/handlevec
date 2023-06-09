use handlevec::IntoMutHandles;

fn main() {
    let my_vec = vec![2, 3, 4, 5, 6, 7, 1];

    let mut my_count = 0;

    println!("{:?}", my_vec);

    let my_vec = my_vec.mutate_vec_maybe(|mut elem| {
        let val = *elem.get()?;
        my_count += val;

        if val > 6 {
            elem.discard()?;
        } else if val < 3 {
            let x = elem.peek_forward_slice(0..)?.len();
            elem.set(x)?;
        }

        Some(())
    });

    println!("{}", my_count);
    println!("{:?}", my_vec);
}
