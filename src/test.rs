#![allow(warnings)]
use tokio::sync::oneshot;
use troupe_macro::actor;

use crate as troupe;

pub struct AType;
pub struct Pattern;
pub struct Return;

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

	#[performance(canonical)]
	impl Funcoot for Olaf {
		fn do_thing(&mut self, a_name: AType, another: Pattern) -> Return {
			self.speak();
		}
	}
}

#[tokio::main]
#[test]
async fn ping() {
	let state = Olaf { alive: true };
	let (actor_handle, _) = Olaf::start(state);
	assert!(actor_handle.do_thing(AType {}, Pattern {}).is_ok());
	//panic!("stop");
}

#[actor]
mod actor {
	use std::sync::Arc;
	use std::time::Instant;

	struct ChainIm {
		next: Option<Arc<dyn Chain<Info = ChainInfo> + Send + Sync>>,
	}

	#[performance(canonical)]
	impl Chain for ChainIm {
		fn poke(&mut self, start: Instant, sender: tokio::sync::oneshot::Sender<()>) {
			match &self.next {
				Some(next) => next.poke(start, sender).unwrap_or_else(|_| panic!()),
				None => println!("{:?}", start.elapsed()),
			}
		}
	}
}

#[tokio::main]
#[test]
async fn chain() {
	let mut actor = ChainIm::start(ChainIm { next: None }).0;

	let begin = Instant::now();

	for _ in 0..1000000 {
		let new_state = ChainIm { next: Some(actor) };
		actor = ChainIm::start(new_state).0;
	}

	let (sender, receiver) = oneshot::channel();

	let mid = Instant::now();
	println!("{:?}", mid - begin);
	actor.poke(begin, sender);

	receiver.await;
	panic!();
}
