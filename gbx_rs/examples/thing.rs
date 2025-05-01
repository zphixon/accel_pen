trait ClassId: Clone {
    const CLASS_ID: u32;
    fn class_id(&self) -> u32 {
        Self::CLASS_ID
    }
}

#[derive(Clone)]
struct A;
impl ClassId for A {
    const CLASS_ID: u32 = 0xAAAA;
}

#[derive(Clone)]
struct B;
impl ClassId for B {
    const CLASS_ID: u32 = 0xBBBB;
}

fn main() {
    use random::Source;
    let items = if random::default().read_u64() % 2 == 0 {
        [
            Box::into_raw(Box::new(A)) as *mut (),
            Box::into_raw(Box::new(B)) as *mut (),
        ]
    } else {
        [
            Box::into_raw(Box::new(B)) as *mut (),
            Box::into_raw(Box::new(A)) as *mut (),
        ]
    };

    for item in items {
        let boxed_a = unsafe { Box::from_raw(item as *mut A) };
        // this is not how it works
        if boxed_a.class_id() == A::CLASS_ID {
            println!("got an A, hooray");
        } else {
            println!("not an A!!!!! SCARY!");
        }
    }
}
