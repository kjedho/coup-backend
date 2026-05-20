use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::card::Card;
use super::game::Game;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Player {
    pub uuid: Uuid,
    pub name: String,
    pub is_alive: bool,
    pub cards: Vec<Card>,
    pub coins: u8,
    pub exchange_cards: Vec<Card>,
}

impl Player {
    pub fn new(uuid: &Uuid, name: &str) -> Self {
        Self {
            uuid: *uuid,
            name: name.to_string(),
            is_alive: true,
            cards: vec![],
            coins: 2,
            exchange_cards: vec![],
        }
    }

    pub fn lose_influence(&mut self, role_name: Option<&str>) -> Result<bool, &'static str> {
        let hidden_count = self.cards.iter().filter(|c| !c.visible).count();
        match hidden_count {
            0 => Err("Cannot lose influence"),
            1 => {
                self.cards
                    .iter_mut()
                    .find(|c| !c.visible)
                    .unwrap()
                    .visible = true;
                self.is_alive = false;
                Ok(true)
            }
            2 => {
                if let Some(name) = role_name {
                    let card = self
                        .cards
                        .iter_mut()
                        .find(|c| !c.visible && format!("{:?}", c.role) == name);
                    match card {
                        Some(c) => {
                            c.visible = true;
                            Ok(true)
                        }
                        None => Err("You don't have that card"),
                    }
                } else {
                    // No choice provided, caller should prompt the player
                    Err("Must choose which card to lose")
                }
            }
            _ => Err("Invalid number of cards"),
        }
    }

    pub fn exchange_draw(&mut self, game: &mut Game) -> Result<Vec<Card>, &'static str> {
        self.exchange_cards.clear();
        for card in self.cards.iter() {
            if !card.visible {
                self.exchange_cards.push(*card);
            }
        }
        for _ in 0..2 {
            let card = game.deck.draw().unwrap();
            self.exchange_cards.push(card);
        }
        Ok(self.exchange_cards.clone())
    }

    pub fn exchange_confirm(
        &mut self,
        game: &mut Game,
        cards: &[Card],
    ) -> Result<bool, &'static str> {
        let hidden_count = self.cards.iter().filter(|c| !c.visible).count();
        if cards.len() != hidden_count {
            return Err("Invalid number of cards selected");
        }
        for card in cards {
            if !self.exchange_cards.contains(card) {
                return Err("Invalid card");
            }
        }
        // Replace hidden cards with selected cards
        let mut card_idx = 0;
        for i in 0..self.cards.len() {
            if !self.cards[i].visible {
                self.cards[i] = cards[card_idx];
                card_idx += 1;
            }
        }
        // Return unselected cards to deck
        for card in self.exchange_cards.iter() {
            if !cards.contains(card) {
                game.deck.return_card(*card).unwrap();
            }
        }
        self.exchange_cards.clear();
        Ok(true)
    }

}
