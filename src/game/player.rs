use std::vec;
use rand::Rng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use super::card::Card;
use super::game::Game;


#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Player {
    pub name: String,
    pub is_alive: bool,
    pub cards: Vec<Card>,
    pub coins: u8,
}

impl Player {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            is_alive: true,
            cards: vec![],
            coins: 2,
        }
    }

    pub fn lose_influence(&mut self) -> Result<bool, &'static str> {
        let cards_remaining = self.cards.iter().filter(|card| !card.visible).count();
        match cards_remaining {
            0 => Err("Cannot lose influence"),
            1 => {
                self.cards.iter_mut().find(|card| !card.visible).unwrap().visible = true;
                self.is_alive = false;
                Ok(true)
            }
            2 => {
                //TODO: give player choice of card to lose in some way
                let mut rng = rand::thread_rng();
                let index = rng.gen_range(0..2);
                self.cards[index].visible = true;
                Ok(true)
            }
            _ => Err("Invalid number of cards"),
        }
    }

    pub fn income(&mut self, game: &mut Game) -> Result<bool, &'static str> {
        if game.coins == 0 {
            return Err("No coins left");
        }
        self.coins += 1;
        game.coins -= 1;
        Ok(true)
    }

    pub fn foreign_aid(&mut self, game: &mut Game) -> Result<bool, &'static str> {
        if game.coins < 2 {
            return Err("Not enough coins left");
        }
        self.coins += 2;
        game.coins -= 2;
        Ok(true)
    }

    pub fn coup(&mut self, game: &mut Game, target: &mut Player) -> Result<bool, &'static str> {
        if self.coins < 7 {
            return Err("Not enough coins to coup");
        }
        if self == target {
            return Err("Cannot coup yourself");
        }
        if !target.is_alive {
            return Err("Target is already dead");
        }
        self.coins -= 7;
        game.coins += 7;
        target.lose_influence()
    }

    pub fn tax(&mut self, game: &mut Game) -> Result<bool, &'static str> {
        if game.coins < 3 {
            return Err("Not enough coins left");
        }
        self.coins += 3;
        game.coins -= 3;
        Ok(true)
    }

    pub fn assassinate(&mut self, game: &mut Game, target: &mut Player) -> Result<bool, &'static str> {
        if self.coins < 3 {
            return Err("Not enough coins to assassinate");
        }
        if self == target {
            return Err("Cannot assassinate yourself");
        }
        if !target.is_alive {
            return Err("Target is already dead");
        }
        self.coins -= 3;
        game.coins += 3;
        target.lose_influence()
    }

    pub fn exchange(&mut self, game: &mut Game) -> Result<bool, &'static str> {
        let mut new_cards = vec![];
        for _ in 0..2 {
            let card = game.deck.draw().unwrap();
            new_cards.push(card);
        }
        self.cards.extend(new_cards);
        // TODO: give player choice of cards to return
        self.cards.shuffle(&mut rand::thread_rng());
        for _ in 0..2 {
            let card = self.cards.pop().unwrap();
            game.deck.return_card(card).unwrap();
        }

        Ok(true)
    }

    pub fn steal(&mut self, target: &mut Player) -> Result<bool, &'static str> {
        if target.coins == 0 {
            return Err("Target has no coins to steal");
        }
        let coins_stolen = std::cmp::min(target.coins, 2);
        self.coins += coins_stolen;
        target.coins -= coins_stolen;
        Ok(true)
    }

    pub fn call_bluff(&mut self, target: &mut Player, card: Card) -> Result<bool, &'static str> {
        if target.cards.contains(&card) {
            self.lose_influence()?;
        } else {
            target.lose_influence()?;
        }
        Ok(true)
    }

}