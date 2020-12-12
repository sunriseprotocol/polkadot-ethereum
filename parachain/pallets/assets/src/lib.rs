//! # Assets
//!
//! The assets pallet provides functionality for handling account balances.
//!
//! ## Overview
//!
//! This module is used by the ETH and ERC20 pallets to store account balances for bridged assets.
//!
//! Each asset is identified by a unique `H160` hash. This is useful for tracking ERC20 tokens which are natively identified by a 20-byte contract address.
//!
//! ## Interface
//!
//! ### Dispatchable Calls
//!
//! - `transfer`: Transferring a balance between accounts.
//!
//! ### Public Functions
//!
//! - `total_issuance`: Retrieve total issuance for a fungible asset.
//! - `balance`: Retrieve an account balance.
//! - `withdraw`: Withdraw from an account.
//! - `deposit`: Deposit into an account.
//! - `transfer`: Transfer between accounts.
//!
#![cfg_attr(not(feature = "std"), no_std)]

use sp_std::prelude::*;
use frame_system::{self as system, ensure_signed};
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage,
	traits::Get,
	dispatch::{DispatchResult, DispatchError},
};

use sp_core::{U256};

use artemis_core::assets::{AssetId, MultiAsset, SingleAsset};
use sp_std::marker;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_storage! {
	trait Store for Module<T: Trait> as Asset {
		pub TotalIssuance get(fn total_issuance): map hasher(blake2_128_concat) AssetId => U256;
		pub Balances get(fn balances): double_map hasher(blake2_128_concat) AssetId, hasher(blake2_128_concat) T::AccountId => U256;
	}
	add_extra_genesis {
		config(balances): Vec<(AssetId, T::AccountId, U256)>;
		build(|config: &GenesisConfig<T>| {
			for &(ref asset_id, ref who, amount) in config.balances.iter() {
				let total_issuance = TotalIssuance::get(asset_id);
				TotalIssuance::insert(asset_id, total_issuance + amount);
				Balances::<T>::insert(asset_id, who, amount);
			}
		});
	}
}

decl_event!(
	pub enum Event<T>
	where
		<T as system::Trait>::AccountId,
	{
		Transferred(AssetId, AccountId, AccountId, U256),
	}
);

decl_error! {
	pub enum Error for Module<T: Trait> {
		TotalIssuanceOverflow,
		TotalIssuanceUnderflow,
		BalanceOverflow,
		InsufficientBalance
	}
}

decl_module! {

	pub struct Module<T: Trait> for enum Call where origin: T::Origin {

		type Error = Error<T>;

		fn deposit_event() = default;

		/// Transfer some free balance to another account.
		// TODO: Calculate weights (#161)
		#[weight = 10]
		pub fn transfer(origin,
						dest: T::AccountId,
						asset_id: AssetId,
						amount: U256) -> DispatchResult {
			let who = ensure_signed(origin)?;
			<Self as MultiAsset<_>>::transfer(asset_id, &who, &dest, amount)
		}

	}
}

impl<T: Trait> MultiAsset<T::AccountId> for Module<T> {

	fn total_issuance(asset_id: AssetId) -> U256 {
		Module::<T>::total_issuance(asset_id)
	}

	fn balance(asset_id: AssetId, who: &T::AccountId) -> U256 {
		Module::<T>::balances(asset_id, who)
	}

	fn deposit(asset_id: AssetId, who: &T::AccountId, amount: U256) -> DispatchResult  {
		if amount.is_zero() {
			return Ok(())
		}
		<Balances<T>>::try_mutate(asset_id, who, |balance| -> Result<(), DispatchError> {
			let current_total_issuance = Self::total_issuance(asset_id);
			let new_total_issuance = current_total_issuance.checked_add(amount)
				.ok_or(Error::<T>::TotalIssuanceOverflow)?;
			*balance = balance.checked_add(amount)
				.ok_or(Error::<T>::BalanceOverflow)?;
			<TotalIssuance>::insert(asset_id, new_total_issuance);
			Ok(())
		})
	}

	fn withdraw(asset_id: AssetId, who: &T::AccountId, amount: U256) -> DispatchResult  {
		if amount.is_zero() {
			return Ok(())
		}
		<Balances<T>>::try_mutate(asset_id, who, |balance| -> Result<(), DispatchError> {
			let current_total_issuance = Self::total_issuance(asset_id);
			let new_total_issuance = current_total_issuance.checked_sub(amount)
				.ok_or(Error::<T>::TotalIssuanceUnderflow)?;
			*balance = balance.checked_sub(amount)
				.ok_or(Error::<T>::InsufficientBalance)?;
			<TotalIssuance>::insert(asset_id, new_total_issuance);
			Ok(())
		})
	}

	fn transfer(
		asset_id: AssetId,
		from: &T::AccountId,
		to: &T::AccountId,
		amount: U256)
	-> DispatchResult {
		if amount.is_zero() || from == to {
			return Ok(())
		}
		<Balances<T>>::try_mutate(asset_id, from, |from_balance| -> DispatchResult {
			<Balances<T>>::try_mutate(asset_id, to, |to_balance| -> DispatchResult {
				*from_balance = from_balance.checked_sub(amount).ok_or(Error::<T>::InsufficientBalance)?;
				*to_balance = to_balance.checked_add(amount).ok_or(Error::<T>::BalanceOverflow)?;
				Ok(())
			})
		})
	}
}


pub struct SingleAssetAdaptor<T, I>(marker::PhantomData<(T, I)>);

impl<T, I> SingleAsset<T::AccountId> for SingleAssetAdaptor<T, I>
where
	T: Trait,
	I: Get<AssetId>,
{

	fn total_issuance() -> U256 {
		Module::<T>::total_issuance(I::get())
	}

	fn balance(who: &T::AccountId) -> U256 {
		Module::<T>::balances(I::get(), who)
	}

	fn deposit(
		who: &T::AccountId,
		amount: U256,
	) -> DispatchResult {
		<Module<T> as MultiAsset<_>>::deposit(I::get(), who, amount)
	}

	fn withdraw(
		who: &T::AccountId,
		amount: U256,
	) -> DispatchResult {
		<Module<T> as MultiAsset<_>>::withdraw(I::get(), who, amount)
	}

	fn transfer(
		source: &T::AccountId,
		dest: &T::AccountId,
		amount: U256,
	) -> DispatchResult {
		<Module<T> as MultiAsset<_>>::transfer(I::get(), source, dest, amount)
	}
}