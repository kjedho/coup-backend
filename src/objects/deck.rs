use serde::{Deserialize, Serialize};
use super::card::{Card, Role};
use strum::IntoEnumIterator;
use rand::{thread_rng, seq::SliceRandom};

const CARD_COPIES: usize = 3;
const TOTAL_CARDS: usize = 15;

#[derive(Debug, Serialize, Deserialize)]
pub struct Deck {
    cards: Vec<Card>,
}

impl Deck {
    pub fn new() -> Self {
        Self {
            cards: Vec::from_iter(Role::iter().map(|role| {
                vec![Card::new(role); CARD_COPIES]
            }).flatten())
        }
    }

    pub fn shuffle(&mut self) {
        self.cards.shuffle(&mut thread_rng());
    }

    pub fn draw(&mut self) -> Option<Card> {
        // pop() removes the last element from the vector and returns it
        let card = self.cards.pop();
        match card {
            Some(mut card) => {
                card.visible = false;
                Some(card)
            }
            _ => None
        }
    }

    pub fn return_card(&mut self, mut card: Card) -> Result<bool, &'static str> {
        if self.cards.len() < TOTAL_CARDS {
            card.visible = false;
            self.cards.push(card);
            Ok(true)
        } else {
            Err("Deck is full")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_deck_initialisation() {
        let deck = Deck::new();
        let expected = vec![
            Card::new(Role::Assassin),
            Card::new(Role::Assassin),
            Card::new(Role::Assassin),
            Card::new(Role::Contessa),
            Card::new(Role::Contessa),
            Card::new(Role::Contessa),
            Card::new(Role::Captain),
            Card::new(Role::Captain),
            Card::new(Role::Captain),
            Card::new(Role::Duke),
            Card::new(Role::Duke),
            Card::new(Role::Duke),
            Card::new(Role::Ambassador),
            Card::new(Role::Ambassador),
            Card::new(Role::Ambassador),
        ];
        assert_eq!(deck.cards, expected);
        assert_eq!(deck.cards.len(), TOTAL_CARDS);
    }

    #[test]
    fn check_draw_card() {
        let mut deck = Deck::new();
        let card = deck.draw();
        assert_eq!(card, Some(Card::new(Role::Ambassador)));
        assert_eq!(deck.cards.len(), TOTAL_CARDS-1);

        deck.cards.last_mut().unwrap().visible = true;
        let card = deck.draw();
        assert_eq!(card, Some(Card::new(Role::Ambassador)));
        assert_eq!(card.unwrap().visible, false);

        deck.cards.clear();
        let card = deck.draw();
        assert_eq!(card, None);
    }

    #[test]
    fn check_return_card() {
        let mut deck = Deck::new();
        let result = deck.return_card(Card::new(Role::Ambassador));
        assert_eq!(result, Err("Deck is full"));
        assert_eq!(deck.cards.len(), TOTAL_CARDS);

        _ = deck.draw();
        let result = deck.return_card(Card::new(Role::Ambassador));
        assert_eq!(result, Ok(true));
        assert_eq!(deck.cards.len(), TOTAL_CARDS);
    }
}