use std::{
    borrow::Borrow, collections::HashMap, env, fs::File, hash::Hash, io::Read, ops::Deref,
    str::FromStr,
};

use pest::{
    iterators::{Pair, Pairs},
    Parser,
};
use pest_derive::Parser;
use rand::seq::SliceRandom;

struct RuleTree<'a> {
    rule: Pair<'a, Rule>,
    inner: Vec<RuleTree<'a>>,
}

fn create_tree(rule: Pair<Rule>) -> RuleTree {
    let mut inner = Vec::new();
    for inner_rule in rule.clone().into_inner() {
        inner.push(create_tree(inner_rule));
    }
    RuleTree { rule, inner }
}

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
    let mut deck = convert_decklist_to_vec(deck_string);

    let mut combo_file = File::open(&args[2]).unwrap();
    let mut combo_string: String = String::new();
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
    let iter = 100_000;
    let hand_size = 5;

    // Implement pot of prosperity as six iterations of size six breaking early if possible
    let mut tree_vec = Vec::new();
    let rules = file.clone().into_iter().collect::<Vec<Pair<Rule>>>();
    for rule in rules {
        tree_vec.push(create_tree(rule));
    }

    for _ in 0..iter {
        deck.shuffle(&mut rng);
        let mut hand = Vec::new();
        for _ in 0..hand_size {
            hand.push(deck.pop().unwrap());
        }

        let mut success = false;
        if hand.contains(&String::from("Pot of Prosperity")) && use_prosp {
            let mut prosp_interim = Vec::new();
            for _ in 0..6 {
                prosp_interim.push(deck.pop().unwrap());
            }

            for c in prosp_interim.iter() {
                hand.push(c.to_string());
                for rule in tree_vec.iter() {
                    if match_rule(&hand, &deck, &rule) == true {
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
            deck.extend(prosp_interim);
        } else {
            for rule in tree_vec.iter() {
                if match_rule(&hand, &deck, &rule) == true {
                    sum += 1;
                    break;
                }
            }
        }
        deck.extend(hand);
    }
    println!("Success Rate: {}%", sum as f32 / iter as f32 * 100.0);
}

fn match_rule(hand: &Vec<String>, deck: &Vec<String>, rule_tree: &RuleTree) -> bool {
    match rule_tree.rule.as_rule() {
        Rule::ident => {
            return hand
                .iter()
                .any(|x| x == rule_tree.rule.as_str().trim_end().trim());
        }
        Rule::contains_ident => {
            let ident = rule_tree.inner.first().unwrap().rule.as_str();
            return hand.iter().any(|x| x.contains(ident.trim_end().trim()));
        }
        Rule::num_ident => {
            let mut iter = rule_tree.inner.iter();
            let ident = iter.next().unwrap();
            let comp = iter.next().unwrap();

            let contains_fn = |ident: &Pair<Rule>| {
                hand.iter()
                    .filter(|x| x.contains(ident.as_str().trim_end()))
                    .count()
            };
            let equals_fn = |ident: &Pair<Rule>| {
                hand.iter()
                    .filter(|x| x.as_str() == ident.as_str().trim_end())
                    .count()
            };

            let use_contains = match ident.rule.as_rule() {
                Rule::ident => false,
                Rule::contains_ident => true,
                _ => unreachable!(),
            };

            match comp.rule.as_rule() {
                Rule::greater => {
                    let num = comp
                        .inner
                        .iter()
                        .next()
                        .unwrap()
                        .rule
                        .as_str()
                        .parse::<usize>()
                        .unwrap();
                    if use_contains {
                        return contains_fn(&ident.rule) > num;
                    } else {
                        return equals_fn(&ident.rule) > num;
                    }
                }
                Rule::less => {
                    let num = comp
                        .inner
                        .iter()
                        .next()
                        .unwrap()
                        .rule
                        .as_str()
                        .parse::<usize>()
                        .unwrap();
                    if use_contains {
                        return contains_fn(&ident.rule) < num;
                    } else {
                        return equals_fn(&ident.rule) < num;
                    }
                }
                Rule::equal => {
                    let num = comp
                        .inner
                        .iter()
                        .next()
                        .unwrap()
                        .rule
                        .as_str()
                        .parse::<usize>()
                        .unwrap();
                    if use_contains {
                        return contains_fn(&ident.rule) == num;
                    } else {
                        return equals_fn(&ident.rule) == num;
                    }
                }
                _ => unreachable!(),
            }
        }
        Rule::not => {
            let s = rule_tree.rule.as_str();
            let b = !match_rule(hand, deck, &rule_tree.inner.first().unwrap());
            //println!("not: {} {}", s, b);
            return b;
        }
        Rule::exp => {
            //println!("exp: {}", rule.as_str());
            let mut result = true;
            for inner_rule in rule_tree.inner.iter() {
                match inner_rule.rule.as_rule() {
                    Rule::and => {
                        result = result && match_rule(hand, deck, &inner_rule);
                    }
                    Rule::or => {
                        result = result || match_rule(hand, deck, &inner_rule);
                    }
                    Rule::ident
                    | Rule::exp
                    | Rule::contains_ident
                    | Rule::not
                    | Rule::num_ident
                    | Rule::pick_multi => {
                        result = match_rule(hand, deck, &inner_rule);
                    }
                    _ => {
                        println!("Unknown rule: {:?}", inner_rule.rule.as_rule());
                    }
                }
            }
            return result;
        }
        Rule::pick_multi => {
            let mut option_iter = rule_tree.inner.iter();
            let mut num = option_iter
                .next()
                .unwrap()
                .rule
                .as_str()
                .parse::<usize>()
                .unwrap();
            let mut options: Vec<&str> = Vec::new();
            for inner_rule in option_iter {
                match inner_rule.rule.as_rule() {
                    Rule::ident => {
                        options.push(inner_rule.rule.as_str());
                    }
                    _ => {
                        println!("Unknown rule: {:?}", inner_rule.rule.as_rule());
                    }
                }
            }

            let mut cards_used = Vec::new();
            for _ in 0..hand.len() {
                cards_used.push(false);
            }

            for option in options {
                let index = hand.iter().position(|x| x.as_str() == option);
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
            return match_rule(hand, deck, &rule_tree.inner.first().unwrap());
        }
        _ => {
            println!("Unknown rule: {:?}", rule_tree.rule.as_rule());
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
            let card = card_raw.trim_end().trim().to_string();
            for _ in 0..num {
                deck.push(String::from(card.clone()));
            }
        }
    }
    deck
}
