use crate::{
  ops::{take::TakeOp, Take},
  prelude::*,
};
use std::borrow::Borrow;
use std::sync::{
  atomic::{AtomicBool, Ordering},
  Arc,
};

/// emit only the first item emitted by an Observable
pub trait First {
  fn first(self) -> TakeOp<Self>
  where
    Self: Sized + Take,
  {
    self.take(1)
  }
}

impl<'a, O> First for O where O: ImplSubscribable {}

/// emit only the first item (or a default item) emitted by an Observable
pub trait FirstOr {
  fn first_or(self, default: Self::Item) -> FirstOrOp<TakeOp<Self>, Self::Item>
  where
    Self: ImplSubscribable,
  {
    FirstOrOp {
      source: self.first(),
      default,
      passed: false,
    }
  }
}

impl<O> FirstOr for O where O: ImplSubscribable {}

pub struct FirstOrOp<S, V> {
  source: S,
  default: V,
  passed: bool,
}

impl<S, T> ImplSubscribable for FirstOrOp<S, T>
where
  T: Borrow<S::Item> + Send + Sync + 'static,
  S: ImplSubscribable,
{
  type Item = S::Item;
  type Err = S::Err;
  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> RxReturn<Self::Err> + Send + Sync + 'static,
    error: Option<impl Fn(&Self::Err) + Send + Sync + 'static>,
    complete: Option<impl Fn() + Send + Sync + 'static>,
  ) -> Box<dyn Subscription + Send + Sync> {
    let next = Arc::new(next);
    let c_next = next.clone();
    let default = self.default;
    let passed = Arc::new(AtomicBool::new(self.passed));
    let c_passed = passed.clone();
    self.source.subscribe_return_state(
      move |v| {
        passed.store(true, Ordering::Relaxed);
        c_next(v)
      },
      error,
      Some(move || {
        if !c_passed.load(Ordering::Relaxed) {
          next(default.borrow());
        }
        if let Some(ref comp) = complete {
          comp();
        }
      }),
    )
  }
}

impl<S, T> Multicast for FirstOrOp<S, T>
where
  T: Send + Sync + 'static,
  S: Multicast<Item = T>,
{
  type Output = FirstOrOp<S::Output, Arc<T>>;
  fn multicast(self) -> Self::Output {
    FirstOrOp {
      source: self.source.multicast(),
      default: Arc::new(self.default),
      passed: self.passed,
    }
  }
}

impl<S, T> Fork for FirstOrOp<S, Arc<T>>
where
  T: Send + Sync + 'static,
  S: Fork<Item = T>,
{
  type Output = FirstOrOp<S::Output, Arc<T>>;
  fn fork(&self) -> Self::Output {
    FirstOrOp {
      source: self.source.fork(),
      default: self.default.clone(),
      passed: self.passed,
    }
  }
}

#[cfg(test)]
mod test {
  use super::{First, FirstOr};
  use crate::prelude::*;
  use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
  };

  #[test]
  fn first() {
    let completed = Arc::new(AtomicBool::new(false));
    let next_count = Arc::new(Mutex::new(0));
    let c_next_count = next_count.clone();
    let c_completed = completed.clone();

    observable::from_range(0..2).first().subscribe_complete(
      move |_| *next_count.lock().unwrap() += 1,
      move || completed.store(true, Ordering::Relaxed),
    );

    assert_eq!(c_completed.load(Ordering::Relaxed), true);
    assert_eq!(*c_next_count.lock().unwrap(), 1);
  }

  #[test]
  fn first_or() {
    let completed = Arc::new(AtomicBool::new(false));
    let next_count = Arc::new(Mutex::new(0));
    let c_completed = completed.clone();
    let c_next_count = next_count.clone();

    observable::from_range(0..2)
      .multicast()
      .fork()
      .first_or(100)
      .subscribe_complete(
        move |_| *next_count.lock().unwrap() += 1,
        move || completed.store(true, Ordering::Relaxed),
      );

    assert_eq!(*c_next_count.lock().unwrap(), 1);
    assert_eq!(c_completed.load(Ordering::Relaxed), true);

    c_completed.store(false, Ordering::Relaxed);
    let completed = c_completed.clone();
    let v = Arc::new(Mutex::new(0));
    let c_v = v.clone();
    observable::empty()
      .multicast()
      .fork()
      .first_or(100)
      .subscribe_complete(
        move |value| *v.lock().unwrap() = *value,
        move || completed.store(true, Ordering::Relaxed),
      );

    assert_eq!(c_completed.load(Ordering::Relaxed), true);
    assert_eq!(*c_v.lock().unwrap(), 100);
  }

  #[test]
  fn first_support_fork() {
    use crate::ops::{First, Fork};
    let value = Arc::new(Mutex::new(0));
    let c_value = value.clone();
    let o = observable::from_range(1..100).first().multicast();
    let o1 = o.fork().first();
    let o2 = o.fork().first();
    o1.subscribe(move |v| *value.lock().unwrap() = *v);
    assert_eq!(*c_value.lock().unwrap(), 1);

    *c_value.lock().unwrap() = 0;
    let value = c_value.clone();
    o2.subscribe(move |v| *value.lock().unwrap() = *v);
    assert_eq!(*c_value.lock().unwrap(), 1);
  }
  #[test]
  fn first_or_support_fork() {
    let default = Arc::new(Mutex::new(0));
    let c_default = default.clone();
    let o = Observable::new(|subscriber| {
      subscriber.complete();
      subscriber.error(&"");
    })
    .first_or(100)
    .multicast();
    let o1 = o.fork().first_or(0);
    let o2 = o.fork().first_or(0);
    o1.subscribe(move |v| *default.lock().unwrap() = *v);
    assert_eq!(*c_default.lock().unwrap(), 100);

    *c_default.lock().unwrap() = 0;
    let default = c_default.clone();
    o2.subscribe(move |v| *default.lock().unwrap() = *v);
    assert_eq!(*c_default.lock().unwrap(), 100);
  }
}
