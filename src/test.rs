use troupe_macro::actor;

use crate as troupe;

#[actor]
mod actor {
    struct Olaf {
        alive: bool,
    }
    impl Olaf {
        fn speak(&mut self) {
            println!("Hello")
        }
    }
    impl Funcoot for Olaf {
        fn do_thing(&mut self) -> Return {
            self.speak();
        }
    }
}

#[tokio::main]
#[test]
async fn ping() {
    let state = Olaf { alive: true };
    let (actor_handle, _) = Olaf::start(state);
    actor_handle
        .send(FuncootPayload::DoThing(()))
        .unwrap_or_else(|_| panic!("oops"));
    panic!("stop");
}
