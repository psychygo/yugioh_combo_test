use core::num;
use std::{collections::HashMap, env, fs::File, io::Read};

use pest::{
    iterators::{Pair, Pairs},
    Parser,
};
use pest_derive::Parser;
use rand::seq::SliceRandom;

#[derive(Parser)]
#[grammar = "yugioh_combo_grammar.pest"]
struct IdentParser;
// cargo build --target x86_64-pc-windows-gnu --release
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        println!("Usage: ./yugioh_combo_test(.exe) <decklist file> <combo file>");
        return;
    }
    let mut deck_file = File::open(&args[1]).unwrap();
    let mut deck_string = String::new();
    deck_file
        .read_to_string(&mut deck_string)
        .expect(format!("Failed to read deck file: {}", &args[1]).as_str());
    println!("{}\n", deck_string);
    let deck = convert_decklist_to_vec(deck_string);
    let mut combo_file = File::open(&args[2]).unwrap();
    let mut combo_string = String::new();
    combo_file
        .read_to_string(&mut combo_string)
        .expect(format!("Failed to read combo file: {}", &args[2]).as_str());

    let mut use_prosp = true;
    if args.len() > 3 {
        for i in 3..args.len() {
            if args[i] == "--ignore-prosperity" {
                use_prosp = false;
            }
        }
    }

    let file = IdentParser::parse(Rule::file, &combo_string).unwrap_or_else(|e| panic!("{}", e));
    // Because ident_list is silent, the iterator will contain idents
    let mut rng = rand::thread_rng();
    let mut sum = 0;
    let iter = 100000;

    // Implement pot of prosperity as six iterations of size six breaking early if possible

    for _ in 0..iter {
        let mut d = deck.clone();
        d.shuffle(&mut rng);
        let mut hand: Vec<String> = d.iter().take(5).map(|x| x.clone()).collect();
        let mut success = false;
        if hand.contains(&String::from("Pot of Prosperity")) && use_prosp {
            let interim: Vec<String> = d.iter().skip(5).take(6).map(|x| x.clone()).collect();
            for c in interim {
                hand.push(c);
                for rule in file.clone().into_iter() {
                    if match_rule(&hand, &d, rule) == true {
                        success = true;
                        break;
                    }
                }
                hand.pop();
                if success {
                    sum += 1;
                    break;
                }
            }
        } else {
            for rule in file.clone().into_iter() {
                if match_rule(&hand, &d, rule) == true {
                    sum += 1;
                    break;
                }
            }
        }
    }
    println!("Success Rate: {}%", sum as f32 / iter as f32 * 100.0);
}

/*used_map needs to hold rule/string of used idents exclusive_ident will attempt to do the match, swapping if possible. exclusive idents will only check against other exclusive idents*/
// Could store the whole subtree using Pair<Rule>, and retry the whole thing when you need to swap

// pick_multi = { '[' ~ ']' } use to make pick x of names in the list, not able to do duplicates of names
// implement by looping through the different options, check if matches, increment and remove if true then move on to the next, breaking early if the required number is met
fn match_rule(hand: &Vec<String>, deck: &Vec<String>, rule: Pair<Rule>) -> bool {
    match rule.as_rule() {
        Rule::ident => {
            return hand.iter().any(|x| x == rule.as_str());
        }
        Rule::contains_ident => {
            let ident = rule.into_inner().next().unwrap().as_str();
            return hand.iter().any(|x| x.contains(ident));
        }
        Rule::num_ident => {
            let mut iter = rule.into_inner();
            let ident = iter.next().unwrap();
            let comp = iter.next().unwrap();

            let contains_fn =
                |ident: Pair<Rule>| hand.iter().filter(|x| x.contains(ident.as_str())).count();
            let equals_fn = |ident: Pair<Rule>| {
                hand.iter()
                    .filter(|x| x.as_str() == ident.as_str().trim_end())
                    .count()
            };

            let use_contains = match ident.as_rule() {
                Rule::ident => false,
                Rule::contains_ident => true,
                _ => unreachable!(),
            };

            match comp.as_rule() {
                Rule::greater => {
                    let num = comp
                        .into_inner()
                        .next()
                        .unwrap()
                        .as_str()
                        .parse::<usize>()
                        .unwrap();
                    if use_contains {
                        return contains_fn(ident) > num;
                    } else {
                        return equals_fn(ident) > num;
                    }
                }
                Rule::less => {
                    let num = comp
                        .into_inner()
                        .next()
                        .unwrap()
                        .as_str()
                        .parse::<usize>()
                        .unwrap();
                    if use_contains {
                        return contains_fn(ident) < num;
                    } else {
                        return equals_fn(ident) < num;
                    }
                }
                Rule::equal => {
                    let num = comp
                        .into_inner()
                        .next()
                        .unwrap()
                        .as_str()
                        .parse::<usize>()
                        .unwrap();
                    if use_contains {
                        return contains_fn(ident) == num;
                    } else {
                        return equals_fn(ident) == num;
                    }
                }
                _ => unreachable!(),
            }
        }
        Rule::not => {
            let s = rule.as_str();
            let b = !match_rule(hand, deck, rule.into_inner().next().unwrap());
            //println!("not: {} {}", s, b);
            return b;
        }
        Rule::exp => {
            //println!("exp: {}", rule.as_str());
            let mut result = true;
            for inner_rule in rule.into_inner() {
                match inner_rule.as_rule() {
                    Rule::and => {
                        result = result && match_rule(hand, deck, inner_rule);
                    }
                    Rule::or => {
                        result = result || match_rule(hand, deck, inner_rule);
                    }
                    Rule::ident
                    | Rule::exp
                    | Rule::contains_ident
                    | Rule::not
                    | Rule::num_ident
                    | Rule::pick_multi => {
                        result = match_rule(hand, deck, inner_rule);
                    }
                    _ => {
                        println!("Unknown rule: {:?}", inner_rule.as_rule());
                    }
                }
            }
            return result;
        }
        Rule::pick_multi => {
            let mut option_iter = rule.into_inner();
            let mut num = option_iter
                .next()
                .unwrap()
                .as_str()
                .parse::<usize>()
                .unwrap();
            let mut options: Vec<String> = Vec::new();
            for inner_rule in option_iter {
                match inner_rule.as_rule() {
                    Rule::ident => {
                        options.push(inner_rule.as_str().to_string());
                    }
                    _ => {
                        println!("Unknown rule: {:?}", inner_rule.as_rule());
                    }
                }
            }

            let mut cards_used = Vec::new();
            for _ in 0..hand.len() {
                cards_used.push(false);
            }

            for option in options {
                let index = hand.iter().position(|x| x == option.as_str());
                if let Some(index) = index {
                    if !cards_used[index] {
                        num -= 1;
                        cards_used[index] = true;
                        if num <= 0 {
                            return true;
                        }
                    }
                }
            }
            return num <= 0;
        }
        Rule::and | Rule::or => {
            return match_rule(hand, deck, rule.into_inner().next().unwrap());
        }
        _ => {
            println!("Unknown rule: {:?}", rule.as_rule());
            return false;
        }
    }
}

fn convert_decklist_to_vec(decklist: String) -> Vec<String> {
    let mut deck = Vec::new();
    for line in decklist.lines() {
        let line_iter = line.chars();
        let card_raw: String = line
            .chars()
            .take(line.len() - 2)
            .collect::<String>()
            .trim_end()
            .to_string();
        if let Some(num) = line_iter.last().unwrap().to_digit(10) {
            let card = card_raw.trim_end().to_string();
            for _ in 0..num {
                deck.push(String::from(card.clone()));
            }
        }
    }
    deck
}
