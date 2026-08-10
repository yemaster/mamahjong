use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use mahjong_core::UserId;

use crate::token::random_token;

/// Tickets are exchanged for a socket within seconds of being issued.
pub(crate) const TICKET_LIFETIME: Duration = Duration::from_secs(30);

/// Bounds how many unconsumed tickets one user can hold.
const MAX_TICKETS_PER_USER: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IssuedTicket {
    pub(crate) ticket: String,
    pub(crate) expires_in: u64,
}

struct Ticket {
    user_id: UserId,
    issued_at: Instant,
    expires_at: Instant,
}

/// Single-use, short-lived credentials for the WebSocket upgrade.
#[derive(Clone)]
pub(crate) struct WsTickets {
    tickets: Arc<Mutex<HashMap<String, Ticket>>>,
}

impl WsTickets {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            tickets: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) fn issue(&self, user_id: UserId) -> Option<IssuedTicket> {
        let ticket = random_token()?;
        let now = Instant::now();
        let mut tickets = self.lock();
        tickets.retain(|_, held| held.expires_at > now);
        self.evict_oldest_of_user(&mut tickets, &user_id);
        tickets.insert(
            ticket.clone(),
            Ticket {
                user_id,
                issued_at: now,
                expires_at: now + TICKET_LIFETIME,
            },
        );
        Some(IssuedTicket {
            ticket,
            expires_in: TICKET_LIFETIME.as_secs(),
        })
    }

    /// Removes the ticket and returns its owner; a ticket never authenticates twice.
    pub(crate) fn consume(&self, ticket: &str) -> Option<UserId> {
        let now = Instant::now();
        let mut tickets = self.lock();
        tickets.retain(|_, held| held.expires_at > now);
        tickets.remove(ticket).map(|held| held.user_id)
    }

    fn evict_oldest_of_user(&self, tickets: &mut HashMap<String, Ticket>, user_id: &UserId) {
        while tickets
            .values()
            .filter(|held| &held.user_id == user_id)
            .count()
            >= MAX_TICKETS_PER_USER
        {
            let Some(oldest) = tickets
                .iter()
                .filter(|(_, held)| &held.user_id == user_id)
                .min_by_key(|(_, held)| held.issued_at)
                .map(|(key, _)| key.clone())
            else {
                return;
            };
            tickets.remove(&oldest);
        }
    }

    /// A poisoned lock only means an unrelated thread panicked; the map stays valid.
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Ticket>> {
        self.tickets.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Default for WsTickets {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use mahjong_core::UserId;

    use super::{MAX_TICKETS_PER_USER, WsTickets};

    #[test]
    fn a_ticket_authenticates_exactly_once() {
        let tickets = WsTickets::new();
        let user = UserId::new();
        let issued = tickets.issue(user.clone()).expect("ticket");

        assert_eq!(tickets.consume(&issued.ticket), Some(user));
        assert_eq!(tickets.consume(&issued.ticket), None);
        assert_eq!(tickets.consume("unknown"), None);
    }

    #[test]
    fn tickets_bind_to_their_own_user() {
        let tickets = WsTickets::new();
        let first = UserId::new();
        let second = UserId::new();
        let issued = tickets.issue(first.clone()).expect("ticket");

        assert_ne!(tickets.consume(&issued.ticket), Some(second));
        assert_eq!(tickets.consume(&issued.ticket), None);
    }

    #[test]
    fn holding_too_many_tickets_drops_the_oldest() {
        let tickets = WsTickets::new();
        let user = UserId::new();
        let issued = (0..=MAX_TICKETS_PER_USER)
            .map(|_| tickets.issue(user.clone()).expect("ticket"))
            .collect::<Vec<_>>();

        assert_eq!(tickets.consume(&issued[0].ticket), None);
        assert_eq!(
            tickets
                .consume(&issued[MAX_TICKETS_PER_USER].ticket)
                .as_ref(),
            Some(&user)
        );
    }
}
