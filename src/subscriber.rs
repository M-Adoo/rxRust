use crate::prelude::*;

/// Implements the Observer trait and Subscription trait. While the Observer is
/// the public API for consuming the values of an Observable, all Observers get
/// converted to a Subscriber, in order to provide Subscription capabilities.
///
#[derive(Clone)]
pub struct Subscriber<O, U> {
  pub(crate) observer: O,
  pub(crate) subscription: U,
}

impl<O> Subscriber<O, LocalSubscription> {
  pub fn local(observer: O) -> Self {
    Subscriber {
      observer,
      subscription: LocalSubscription::default(),
    }
  }
}

impl<O> Subscriber<O, SharedSubscription> {
  pub fn shared(observer: O) -> Self {
    Subscriber {
      observer,
      subscription: SharedSubscription::default(),
    }
  }
}

impl<S, U> IntoShared for Subscriber<S, U>
where
  S: IntoShared,
  U: IntoShared,
{
  type Shared = Subscriber<S::Shared, U::Shared>;
  fn to_shared(self) -> Self::Shared {
    Subscriber {
      subscription: self.subscription.to_shared(),
      observer: self.observer.to_shared(),
    }
  }
}

impl<Item, Err, S, U> Observer<Item, Err> for Subscriber<S, U>
where
  S: Observer<Item, Err>,
  U: SubscriptionLike,
{
  fn next(&mut self, v: Item) {
    if !self.subscription.is_closed() {
      self.observer.next(v)
    }
  }

  fn complete(&mut self) {
    if !self.subscription.is_closed() {
      self.observer.complete();
      self.subscription.unsubscribe()
    }
  }

  fn error(&mut self, err: Err) {
    if !self.subscription.is_closed() {
      self.observer.error(err);
      self.subscription.unsubscribe();
    }
  }
}

impl<O, U> SubscriptionLike for Subscriber<O, U>
where
  U: SubscriptionLike,
{
  #[inline(always)]
  fn unsubscribe(&mut self) { self.subscription.unsubscribe(); }

  #[inline(always)]
  fn is_closed(&self) -> bool { self.subscription.is_closed() }

  #[inline(always)]
  fn inner_addr(&self) -> *const () { self.subscription.inner_addr() }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use std::sync::{Arc, Mutex};

  #[test]
  fn shared_next_complete() {
    let (next, _, complete, mut subscriber) = shared_subscriber_creator();
    subscriber.next(1);
    subscriber.next(2);
    subscriber.complete();
    subscriber.next(3);
    subscriber.next(4);
    assert_eq!(*next.lock().unwrap(), 2);
    assert_eq!(*complete.lock().unwrap(), 1);
  }

  #[test]
  fn shared_err_complete() {
    let (next, error, _, mut subscriber) = shared_subscriber_creator();
    subscriber.next(1);
    subscriber.next(2);
    subscriber.error(());
    subscriber.next(3);
    subscriber.next(4);

    assert_eq!(*next.lock().unwrap(), 2);
    assert_eq!(*error.lock().unwrap(), 1);
  }

  type SubscriberInfo<O> =
    (Arc<Mutex<i32>>, Arc<Mutex<i32>>, Arc<Mutex<i32>>, O);
  fn shared_subscriber_creator() -> SubscriberInfo<impl Observer<i32, ()>> {
    let next = Arc::new(Mutex::new(0));
    let err = Arc::new(Mutex::new(0));
    let complete = Arc::new(Mutex::new(0));

    (
      next.clone(),
      err.clone(),
      complete.clone(),
      Subscriber::shared(SubscribeAll::new(
        move |_| *next.lock().unwrap() += 1,
        move |_| *err.lock().unwrap() += 1,
        move || *complete.lock().unwrap() += 1,
      ))
      .to_shared(),
    )
  }

  #[test]
  fn next_and_complete() {
    let mut next = 0;
    let mut complete = 0;

    let mut subscriber = Subscriber::local(SubscribeAll::new(
      |_: &_| next += 1,
      |_: &i32| {},
      || complete += 1,
    ));

    subscriber.next(&1);
    subscriber.next(&2);
    subscriber.complete();
    subscriber.next(&3);
    subscriber.next(&4);
    assert_eq!(next, 2);
    assert_eq!(complete, 1);
  }

  #[test]
  fn next_and_error() {
    let mut next = 0;
    let mut err = 0;

    let mut subscriber = Subscriber::local(SubscribeErr::new(
      |_: &_| next += 1,
      |_: &()| err += 1,
    ));

    subscriber.next(&1);
    subscriber.next(&2);
    subscriber.error(&());
    subscriber.next(&3);
    subscriber.next(&4);

    assert_eq!(next, 2);
    assert_eq!(err, 1);
  }
}
