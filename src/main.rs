use std::{env, fs::File, hash::Hash, io::Read};

use pest::{iterators::Pair, Parser};
use pest_derive::Parser;

#[derive(Clone)]
struct Hand<'b> {
    main: &'b [String],
    prosp_interim: &'b [String],
    prosp_idx: usize,
    iter_idx: usize,
    should_fail: bool,
}

impl<'b> Iterator for Hand<'b> {
    type Item = &'b String;
    fn next(&mut self) -> Option<Self::Item> {
        if self.iter_idx < self.main.len() {
            let foo = &self.main[self.iter_idx];
            self.iter_idx += 1;
            return Some(foo);
        } else {
            if self.prosp_idx < self.prosp_interim.len() && !self.should_fail {
                self.should_fail = true;
                return Some(&self.prosp_interim[self.prosp_idx]);
            } else {
                return None;
            }
        }
    }
}

impl Hand<'_> {
    fn next_hand(&mut self) -> bool {
        self.prosp_idx += 1;
        self.should_fail = false;
        self.iter_idx = 0;
        if self.prosp_idx < self.prosp_interim.len() {
            true
        } else {
            false
        }
    }
    fn len(&self) -> usize {
        let mut result = self.main.len();
        if self.prosp_interim.len() != 0 {
            result += 1;
        }
        result
    }
}

struct RuleTree<'a> {
    rule: Pair<'a, Rule>,
    inner: Vec<RuleTree<'a>>,
    normalized_string: Option<String>,
    rule_enum: Rule,
}

fn create_tree(rule: Pair<Rule>) -> RuleTree {
    let mut inner = Vec::new();
    for inner_rule in rule.clone().into_inner() {
        inner.push(create_tree(inner_rule));
    }

    match rule.as_rule() {
        Rule::ident | Rule::contains_ident => RuleTree {
            normalized_string: Some(normalize_string(rule.as_str().to_string())),
            rule_enum: rule.as_rule(),
            rule,
            inner,
        },
        _ => RuleTree {
            rule_enum: rule.as_rule(),
            rule,
            inner,
            normalized_string: None,
        },
    }
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
    let prosp = String::from("Pot of Prosperity");

    // Because ident_list is silent, the iterator will contain idents
    //let mut rng = rand::thread_rng();
    let mut sum = 0;
    let iter = 1_000_000;
    let hand_size = 5;

    // Implement pot of prosperity as six iterations of size six breaking early if possible
    let mut tree_vec = Vec::new();
    let rules = file.clone().into_iter().collect::<Vec<Pair<Rule>>>();
    for rule in rules {
        tree_vec.push(create_tree(rule));
    }

    for _ in 0..iter {
        //let start = Instant::now();
        //deck.shuffle(&mut rng);
        fastrand::shuffle(&mut deck);
        let mut hand = Hand {
            main: &deck[..hand_size],
            prosp_interim: &[],
            prosp_idx: 0,
            iter_idx: 0,
            should_fail: false,
        };
        if use_prosp && deck[..hand_size].contains(&prosp) {
            hand.prosp_interim = &deck[hand_size..hand_size + 6]
        }

        loop {
            if tree_vec.iter().any(|rule| match_rule(hand.clone(), &rule)) {
                sum += 1;
                break;
            } else {
                if !hand.next_hand() {
                    break;
                }
            }
        }
    }
    println!("Success Rate: {}%", sum as f32 / iter as f32 * 100.0);
}

fn match_rule(hand: Hand, rule_tree: &RuleTree) -> bool {
    match rule_tree.rule_enum {
        Rule::ident => {
            return hand.into_iter().any(|x| {
                x.as_str()
                    == rule_tree
                        .normalized_string
                        .as_ref()
                        .expect("Invariant broken: ident must set normalized_string")
            });
        }
        Rule::contains_ident => {
            return hand.into_iter().any(|x| {
                x.as_str().contains(
                    rule_tree
                        .normalized_string
                        .as_ref()
                        .expect("Invariant broken: Contains ident must set normalized_string"),
                )
            });
        }
        Rule::num_ident => {
            let mut iter = rule_tree.inner.iter();
            let ident = iter.next().unwrap();
            let comp = iter.next().unwrap();

            let contains_fn = |ident: &RuleTree, hand: Hand| {
                hand.into_iter()
                    .filter(|x| {
                        x.contains(
                            ident
                                .normalized_string
                                .as_ref()
                                .expect("Invariant Broken on ident")
                                .as_str(),
                        )
                    })
                    .count()
            };
            let equals_fn = move |ident: &RuleTree, hand: Hand| {
                hand.into_iter()
                    .filter(|x| {
                        x.as_str()
                            == ident
                                .normalized_string
                                .as_ref()
                                .expect("Invariant Broken on ident")
                                .as_str()
                    })
                    .count()
            };

            let use_contains = match ident.rule.as_rule() {
                Rule::ident => false,
                Rule::contains_ident => true,
                _ => unreachable!(),
            };

            let target_num = comp
                .inner
                .iter()
                .next()
                .unwrap()
                .rule
                .as_str()
                .parse::<usize>()
                .unwrap();

            let num_found;

            if use_contains {
                num_found = contains_fn(&ident, hand);
            } else {
                num_found = equals_fn(&ident, hand);
            }

            match comp.rule.as_rule() {
                Rule::greater => num_found > target_num,
                Rule::less => num_found < target_num,
                Rule::equal => num_found == target_num,
                _ => unreachable!(),
            }
        }
        Rule::not => {
            let b = !match_rule(hand, &rule_tree.inner.first().unwrap());
            return b;
        }
        Rule::exp => {
            let mut result = true;
            for inner_rule in rule_tree.inner.iter() {
                let hand = hand.clone();
                match inner_rule.rule_enum {
                    Rule::and => {
                        result = result && match_rule(hand, &inner_rule);
                    }
                    Rule::or => {
                        result = result || match_rule(hand, &inner_rule);
                    }
                    Rule::ident
                    | Rule::exp
                    | Rule::contains_ident
                    | Rule::not
                    | Rule::num_ident
                    | Rule::pick_multi => {
                        result = match_rule(hand, &inner_rule);
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
                let hand = hand.clone();
                let index = hand.into_iter().position(|x| x.as_str() == option);
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
            return match_rule(hand, &rule_tree.inner.first().unwrap());
        }
        _ => {
            println!("Unknown rule: {:?}", rule_tree.rule.as_rule());
            return false;
        }
    }
}

fn convert_decklist_to_vec(decklist: String) -> Vec<String> {
    let mut deck = Vec::with_capacity(60);
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
                deck.push(card.clone());
            }
        }
    }
    deck
}

fn normalize_string(str: String) -> String {
    let mut new_str = String::with_capacity(str.len());
    let mut keep = true;
    for c in str.chars() {
        if c == '\\' {
            keep = false;
            continue;
        } else if !keep {
            keep = true;
            continue;
        }
        new_str.push(c);
    }
    return new_str.trim_end().trim().to_string();
}
