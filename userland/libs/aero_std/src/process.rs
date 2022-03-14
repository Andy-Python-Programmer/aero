use core::fmt::Debug;

#[lang = "termination"]
pub trait Termination {
    fn report(self) -> i32;
}

impl Termination for () {
    fn report(self) -> i32 {
        0
    }
}

impl Termination for ! {
    fn report(self) -> i32 {
        unreachable!()
    }
}

impl<E: Debug> Termination for Result<(), E> {
    fn report(self) -> i32 {
        match self {
            Ok(()) => 0,
            Err(err) => Err::<!, _>(err).report(),
        }
    }
}

impl<E: Debug> Termination for Result<!, E> {
    fn report(self) -> i32 {
        match self {
            Ok(_) => unreachable!(),
            Err(err) => {
                println!("Error: {:?}", err);
                1
            }
        }
    }
}
