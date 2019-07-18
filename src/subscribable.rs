use crate::prelude::*;

pub enum OState<E> {
  Next,
  Complete,
  Err(E),
}

pub trait ImplSubscribable<'a>: Sized {
  /// The type of the elements being emitted.
  type Item;
  // The type of the error may propagating.
  type Err;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Box<dyn Subscription + 'a>;
}

pub trait Subscribable<'a>: ImplSubscribable<'a> {
  /// Invokes an execution of an Observable and registers Observer handlers for
  /// notifications it will emit.
  ///
  /// * `error`: A handler for a terminal event resulting from an error.
  /// * `complete`: A handler for a terminal event resulting from successful
  /// completion.
  ///
  fn subscribe_err_complete(
    self,
    next: impl Fn(&Self::Item) + 'a,
    error: impl Fn(&Self::Err) + 'a,
    complete: impl Fn() + 'a,
  ) -> Box<dyn Subscription + 'a> {
    self.subscribe_return_state(
      move |v| {
        next(v);
        OState::Next
      },
      Some(error),
      Some(complete),
    )
  }

  fn subscribe_err(
    self,
    next: impl Fn(&Self::Item) + 'a,
    error: impl Fn(&Self::Err) + 'a,
  ) -> Box<dyn Subscription + 'a> {
    let complete: Option<fn()> = None;
    self.subscribe_return_state(
      move |v| {
        next(v);
        OState::Next
      },
      Some(error),
      complete,
    )
  }

  fn subscribe_complete(
    self,
    next: impl Fn(&Self::Item) + 'a,
    complete: impl Fn() + 'a,
  ) -> Box<dyn Subscription + 'a>
  where
    Self::Err: 'a,
  {
    let err: Option<fn(&Self::Err)> = None;
    self.subscribe_return_state(
      move |v| {
        next(v);
        OState::Next
      },
      err,
      Some(complete),
    )
  }

  fn subscribe(
    self,
    next: impl Fn(&Self::Item) + 'a,
  ) -> Box<dyn Subscription + 'a>
  where
    Self::Err: 'a,
  {
    let complete: Option<fn()> = None;
    let err: Option<fn(&Self::Err)> = None;
    self.subscribe_return_state(
      move |v| {
        next(v);
        OState::Next
      },
      err,
      complete,
    )
  }
}

impl<'a, S: ImplSubscribable<'a>> Subscribable<'a> for S {}
