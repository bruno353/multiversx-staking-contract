#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[derive(NestedEncode, NestedDecode, TopEncode, TopDecode, PartialEq, Eq, TypeAbi, Clone)]
pub struct Stake<M: ManagedTypeApi> {
    amount: BigUint<M>,
    reward_debt: BigUint<M>,
}

impl<M: ManagedTypeApi> Default for Stake<M> {
    fn default() -> Self {
        Self {
            amount: BigUint::zero(),
            reward_debt: BigUint::zero(),
        }
    }
}

pub const REWARD_PER_SECOND: u64 = 300; // Example value - meant to be 0.0003 ELGD, as requested


#[multiversx_sc::contract]
pub trait StakingContract {

    #[view(getStakingPosition)]
    #[storage_mapper("stakes")]
    fn stakes(&self, donor: &ManagedAddress) -> SingleValueMapper<Stake<Self::Api>>;

    #[storage_mapper("totalStaked")]
    fn total_staked(&self) -> SingleValueMapper<BigUint>;

    #[storage_mapper("accRewardPerShare")]
    fn acc_reward_per_share(&self) -> SingleValueMapper<BigUint>;

    #[storage_mapper("lastRewardTime")]
    fn last_reward_time(&self) -> SingleValueMapper<u64>;

    #[init]
    fn init(&self) {
        self.last_reward_time().set(&self.blockchain().get_block_timestamp());
    }

    #[view(getTotalStaked)]
    #[storage_mapper("totalStaked")]
    fn get_total_staked(&self) -> SingleValueMapper<BigUint>;

    #[endpoint]
    #[payable("EGLD")]
    fn stake(&self) {
        let payment_amount = self.call_value().egld_value().clone_value();
        require!(payment_amount > 0, "Must pay more than 0");

        let caller = self.blockchain().get_caller();
        self.update_pool(); 

        let mut stake = if self.stakes(&caller).is_empty() {
            Stake::default()
        } else {
            self.stakes(&caller).get()
        };

        if stake.amount > 0 {
            let pending_reward = &stake.amount * &self.acc_reward_per_share().get() - &stake.reward_debt;
            self.send_rewards(&caller, &pending_reward);
        }
    
        stake.amount += &payment_amount;
        stake.reward_debt = &stake.amount * &self.acc_reward_per_share().get();
        self.stakes(&caller).set(&stake);
        self.total_staked().update(|val| *val += &payment_amount);
    }

    fn update_pool(&self) {
        let current_time = self.blockchain().get_block_timestamp();
        let last_reward_time = self.last_reward_time().get();
        if current_time <= last_reward_time {
            return;
        }

        let total_staked = self.total_staked().get();
        if total_staked == 0 {
            self.last_reward_time().set(&current_time);
            return;
        }

        let elapsed_time = current_time - last_reward_time;
        let reward = BigUint::from(elapsed_time) * REWARD_PER_SECOND;
        let acc_reward_per_share = self.acc_reward_per_share().get() + reward / &total_staked;
        self.acc_reward_per_share().set(&acc_reward_per_share);
        self.last_reward_time().set(&current_time);
    }

    fn send_rewards(&self, to: &ManagedAddress, amount: &BigUint) {
        if amount > &0 {
            self.send().direct_egld(to, &amount);
        }
    }

    #[endpoint]
    fn claim_rewards(&self) {
        let caller = self.blockchain().get_caller();
        self.update_pool();

        let mut stake = self.stakes(&caller).get();
        require!(stake.amount > 0, "No staked amount");

        let pending_reward = &stake.amount * &self.acc_reward_per_share().get() - &stake.reward_debt;
        require!(pending_reward > 0, "No rewards to claim");

        stake.reward_debt = &stake.amount * &self.acc_reward_per_share().get();
        self.stakes(&caller).set(&stake);

        self.send_rewards(&caller, &pending_reward);
    }

    #[endpoint]
    fn unstake(&self) {
        let caller = self.blockchain().get_caller();
        self.update_pool();
    
        let mut stake = self.stakes(&caller).get();
        require!(stake.amount > 0, "No staked amount");
    
        let pending_reward = &stake.amount * &self.acc_reward_per_share().get() - &stake.reward_debt;
        
        if pending_reward > 0 {
            self.send_rewards(&caller, &pending_reward);
        }
    
        let unstake_amount = stake.amount.clone();
    
        stake.amount = BigUint::zero();
        stake.reward_debt = BigUint::zero();
        self.stakes(&caller).set(&stake);
    
        self.total_staked().update(|val| *val -= &unstake_amount);
    
        self.send().direct_egld(&caller, &unstake_amount);
    }

    #[endpoint]
    #[payable("EGLD")]
    fn fundMe(&self) {
    }

}