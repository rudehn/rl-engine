//! Who an effect is done by, as plain values.

/// Who a landing is done by: the carrier itself when the thing is used as
/// itself or answers as an actor (an aura is its bearer's own), and
/// otherwise whoever used it, or the carrier when nobody did.
///
/// `A` is whoever can use something, so the rule needs no entities.
pub fn landing_user<A: Copy>(as_itself: bool, is_actor: bool, carrier: A, by: Option<A>) -> A {
    if as_itself || is_actor { carrier } else { by.unwrap_or(carrier) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thing_used_as_itself_or_an_actor_answering_lands_as_its_own_user() {
        assert_eq!(landing_user(true, false, 1, Some(2)), 1);
        assert_eq!(landing_user(false, true, 1, Some(2)), 1);
        assert_eq!(landing_user(true, true, 1, None), 1);
    }

    #[test]
    fn a_carried_thing_lands_as_whoever_used_it_and_as_itself_when_nobody_did() {
        assert_eq!(landing_user(false, false, 1, Some(2)), 2);
        assert_eq!(landing_user(false, false, 1, None), 1);
    }
}
