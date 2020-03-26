use crate::prelude::*;
use observable::observable_proxy_impl;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Config to define leading and trailing behavior for throttle
#[derive(PartialEq, Clone, Copy)]
pub enum ThrottleEdge {
  Tailing,
  Leading,
}

#[derive(Clone)]
pub struct ThrottleTimeOp<S> {
  pub(crate) source: S,
  pub(crate) duration: Duration,
  pub(crate) edge: ThrottleEdge,
}

observable_proxy_impl!(ThrottleTimeOp, S);

impl<Item, Err, S, Unsub> SharedObservable for ThrottleTimeOp<S>
where
  S: for<'r> LocalObservable<'r, Item = Item, Err = Err, Unsub = Unsub>,
  Item: Clone + Send + 'static,
  Unsub: SubscriptionLike + 'static,
{
  type Unsub = Unsub;
  fn actual_subscribe<
    O: Observer<Self::Item, Self::Err> + Send + Sync + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> Self::Unsub {
    let Self {
      source,
      duration,
      edge,
    } = self;
    let mut subscription = LocalSubscription::default();
    subscription.add(subscriber.subscription.clone());
    source.actual_subscribe(Subscriber {
      observer: ThrottleTimeObserver(Arc::new(Mutex::new(
        InnerThrottleTimeObserver {
          observer: subscriber.observer,
          edge,
          delay: duration,
          trailing_value: None,
          throttled: None,
          subscription: subscriber.subscription,
        },
      ))),
      subscription,
    })
  }
}

// Fix me. For now, rust generic specialization is not full finished. we can't
// impl two SharedObservable for ThrottleTimeOp<S>, so we must wrap `S` with
// Shared. And this mean's if any ThrottleTimeOp's upstream just support shared
// subscribe, user must call `to_shared` before `throttle_time`. So,
// ```rust ignore
// observable::interval(Duration::from_millis(1))
//   .throttle_time(Duration::from_millis(9), ThrottleEdge::Leading)
//   .to_shared()
//   .subscribe(move |v| println!("{}", v));
// ```
// this code will not work, must write like this:
// ```rust
// observable::interval(Duration::from_millis(1))
//   .throttle_time(Duration::from_millis(9), ThrottleEdge::Leading)
//   .to_shared()
//   .subscribe(move |v| println!("{}", v));
// ```
impl<S> SharedObservable for ThrottleTimeOp<Shared<S>>
where
  S: SharedObservable,
  S::Item: Clone + Send + 'static,
{
  type Unsub = S::Unsub;
  fn actual_subscribe<
    O: Observer<Self::Item, Self::Err> + Sync + Send + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> S::Unsub {
    let Self {
      source,
      duration,
      edge,
    } = self;
    let Subscriber {
      observer,
      subscription,
    } = subscriber;
    source.0.actual_subscribe(Subscriber {
      observer: ThrottleTimeObserver(Arc::new(Mutex::new(
        InnerThrottleTimeObserver {
          observer,
          edge,
          delay: duration,
          trailing_value: None,
          throttled: None,
          subscription: subscription.clone(),
        },
      ))),
      subscription,
    })
  }
}

struct InnerThrottleTimeObserver<O, Item> {
  observer: O,
  edge: ThrottleEdge,
  delay: Duration,
  trailing_value: Option<Item>,
  throttled: Option<SharedSubscription>,
  subscription: SharedSubscription,
}

pub struct ThrottleTimeObserver<O, Item>(
  Arc<Mutex<InnerThrottleTimeObserver<O, Item>>>,
);

impl<O, Item, Err> Observer<Item, Err> for ThrottleTimeObserver<O, Item>
where
  O: Observer<Item, Err> + Send + 'static,
  Item: Clone + Send + 'static,
{
  fn next(&mut self, value: Item) {
    let mut inner = self.0.lock().unwrap();
    if inner.edge == ThrottleEdge::Tailing {
      inner.trailing_value = Some(value.clone());
    }

    if inner.throttled.is_none() {
      let c_inner = self.0.clone();
      let subscription = Schedulers::ThreadPool.schedule(
        move |_, _| {
          let mut inner = c_inner.lock().unwrap();
          if let Some(v) = inner.trailing_value.take() {
            inner.observer.next(v);
          }
          if let Some(mut throttled) = inner.throttled.take() {
            throttled.unsubscribe();
            inner.subscription.remove(&throttled);
          }
        },
        Some(inner.delay),
        (),
      );
      inner.subscription.add(subscription.clone());
      inner.throttled = Some(subscription);
      if inner.edge == ThrottleEdge::Leading {
        inner.observer.next(value);
      }
    }
  }

  fn error(&mut self, err: Err) {
    let mut inner = self.0.lock().unwrap();
    inner.observer.error(err)
  }

  fn complete(&mut self) {
    let mut inner = self.0.lock().unwrap();
    if let Some(value) = inner.trailing_value.take() {
      inner.observer.next(value);
    }
    inner.observer.complete();
  }
}

#[test]
fn smoke() {
  let x = Arc::new(Mutex::new(vec![]));
  let x_c = x.clone();

  let interval = observable::interval(Duration::from_millis(5));
  let throttle_subscribe = |edge| {
    let x = x.clone();
    interval
      .clone()
      .to_shared()
      .throttle_time(Duration::from_millis(48), edge)
      .to_shared()
      .subscribe(move |v| x.lock().unwrap().push(v))
  };

  // tailing throttle
  let mut sub = throttle_subscribe(ThrottleEdge::Tailing);
  std::thread::sleep(Duration::from_millis(520));
  sub.unsubscribe();
  assert_eq!(
    x_c.lock().unwrap().clone(),
    vec![9, 19, 29, 39, 49, 59, 69, 79, 89, 99]
  );

  // leading throttle
  x_c.lock().unwrap().clear();
  throttle_subscribe(ThrottleEdge::Leading);
  std::thread::sleep(Duration::from_millis(520));
  assert_eq!(
    x_c.lock().unwrap().clone(),
    vec![0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100]
  );
}

#[test]
fn fork_and_shared() {
  observable::of(0..10)
    .throttle_time(Duration::from_nanos(1), ThrottleEdge::Leading)
    .to_shared()
    .to_shared()
    .subscribe(|_| {});
}
