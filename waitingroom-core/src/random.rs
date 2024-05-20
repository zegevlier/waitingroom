use std::{cell::RefCell, rc::Rc};

use rand::{seq::SliceRandom, Rng, RngCore, SeedableRng};

pub trait RandomProvider {
    /// Returns a random u64.
    fn random_u64(&self) -> u64;

    fn shuffle<T>(&self, slice: &mut [T]);
}

#[derive(Debug)]
pub struct TrueRandomProvider;

impl TrueRandomProvider {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for TrueRandomProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RandomProvider for TrueRandomProvider {
    fn random_u64(&self) -> u64 {
        rand::random()
    }

    fn shuffle<T>(&self, slice: &mut [T]) {
        slice.shuffle(&mut rand::thread_rng());
    }
}

#[derive(Clone)]
pub struct DeterministicRandomProvider {
    rand: Rc<RefCell<rand_chacha::ChaCha8Rng>>,
}

impl DeterministicRandomProvider {
    pub fn new(seed: u64) -> Self {
        DeterministicRandomProvider {
            rand: Rc::new(RefCell::new(rand_chacha::ChaCha8Rng::seed_from_u64(seed))),
        }
    }

    pub fn ensure_same_distance(&self, other: &Self) {
        assert_eq!(
            self.rand.try_borrow().unwrap().get_word_pos(),
            other.rand.try_borrow().unwrap().get_word_pos()
        );
        assert_eq!(
            self.rand.try_borrow().unwrap().get_seed(),
            other.rand.try_borrow().unwrap().get_seed()
        );
    }
}

impl RandomProvider for DeterministicRandomProvider {
    fn random_u64(&self) -> u64 {
        log::debug!("DeterministicRandomProvider::random");
        self.rand.try_borrow_mut().unwrap().next_u64()
    }

    fn shuffle<T>(&self, slice: &mut [T]) {
        log::debug!("DeterministicRandomProvider::shuffle");
        for i in 0..slice.len() {
            let j = self
                .rand
                .try_borrow_mut()
                .unwrap()
                .gen_range(0..slice.len());
            slice.swap(i, j);
        }
    }
}
