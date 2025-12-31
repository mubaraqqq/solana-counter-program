use borsh::{ BorshDeserialize, BorshSerialize };
use solana_program::{
    account_info::{ AccountInfo, next_account_info },
    entrypoint::{ ProgramResult, entrypoint },
    msg,
    program::invoke,
    program_error::ProgramError,
    pubkey::Pubkey,
    system_instruction,
    sysvar::{ Sysvar, rent::Rent },
};

entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8]
) -> ProgramResult {
    let instruction = CounterInstruction::try_from_slice(instruction_data).map_err(
        |_| ProgramError::InvalidInstructionData
    )?;

    match instruction {
        CounterInstruction::InitializeCounter { initial_value } => {
            process_initialize_counter(program_id, accounts, initial_value)?;
        }
        CounterInstruction::IncrementCounter => {
            process_increment_counter(program_id, accounts)?;
        }
    }

    Ok(())
}

fn process_initialize_counter(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    initial_value: u64
) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();

    let counter_account = next_account_info(accounts_iter)?;
    let payer_account = next_account_info(accounts_iter)?;
    let system_program = next_account_info(accounts_iter)?;

    let account_space: u64 = 8;

    let rent = Rent::get()?;
    let required_lamports = rent.minimum_balance(account_space as usize);

    invoke(
        &system_instruction::create_account(
            payer_account.key,
            counter_account.key,
            required_lamports,
            account_space,
            program_id
        ),
        &[payer_account.clone(), counter_account.clone(), system_program.clone()]
    )?;

    let counter_data = CounterAccount {
        count: initial_value,
    };

    let mut account_data = &mut counter_account.data.borrow_mut()[..];
    counter_data.serialize(&mut account_data)?;

    msg!("Counter initialized with value: {}", initial_value);

    Ok(())
}

fn process_increment_counter(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();

    let counter_account = next_account_info(accounts_iter)?;
    if counter_account.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }

    let mut data = counter_account.data.borrow_mut();
    let mut counter_data: CounterAccount = CounterAccount::try_from_slice(&data)?;
    counter_data.count = counter_data.count.checked_add(1).ok_or(ProgramError::InvalidAccountData)?;

    counter_data.serialize(&mut &mut data[..])?;

    msg!("Counter incremented to: {}", counter_data.count);

    Ok(())
}

#[derive(BorshDeserialize, BorshSerialize, Debug)]
pub struct CounterAccount {
    pub count: u64,
}

#[derive(BorshDeserialize, BorshSerialize, Debug)]
pub enum CounterInstruction {
    InitializeCounter {
        initial_value: u64,
    },
    IncrementCounter,
}

#[cfg(test)]
mod test {
    use super::*;
    use litesvm::{ LiteSVM };
    use solana_sdk::{
        account::ReadableAccount,
        instruction::{ AccountMeta, Instruction },
        message::Message,
        signature::{ Keypair, Signer },
        system_program,
        transaction::Transaction,
    };

    #[test]
    fn test_counter_program() {
        let mut svm = LiteSVM::new();
        let payer = Keypair::new();
        let counter_keypair = Keypair::new();
        let initial_value: u64 = 42;

        let program_keypair = Keypair::new();
        let program_id = program_keypair.pubkey();

        svm.add_program_from_file(program_id, "target/deploy/counter_program.so").expect(
            "Failed to load program"
        );

        svm.airdrop(&payer.pubkey(), 1_000_000).expect("Failed to airdrop");

        println!("Testing counter initialization");
        // msg!("Test message");

        let init_instruction_data = borsh
            ::to_vec(&(CounterInstruction::InitializeCounter { initial_value }))
            .expect("Failed to serialize instruction");
        let initialize_instruction = Instruction::new_with_bytes(
            program_id,
            &init_instruction_data,
            vec![
                AccountMeta::new(counter_keypair.pubkey(), true),
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(system_program::id(), false)
            ]
        );

        let message = Message::new(&[initialize_instruction], Some(&payer.pubkey()));
        let transaction = Transaction::new(
            &[&payer, &counter_keypair],
            message,
            svm.latest_blockhash()
        );
        let result = svm.send_transaction(transaction);
        assert!(result.is_ok(), "Initialize transaction should succeed");

        let logs = result.unwrap().logs;
        println!("Transaction logs:\n{:#?}", logs);
        //

        let account = svm
            .get_account(&counter_keypair.pubkey())
            .expect("Failed to get counter account");
        let counter = CounterAccount::try_from_slice(account.data()).expect(
            "Failed to deserialize counter data"
        );

        assert_eq!(counter.count, 42);
        println!("Counter initialized successfully with value: {}", counter.count);

        println!("Testing counter incremenet...");
        let increment_instruction_data = borsh
            ::to_vec(&CounterInstruction::IncrementCounter)
            .expect("Failed to serialize instruction");
        let increment_instruction = Instruction::new_with_bytes(
            program_id,
            &increment_instruction_data,
            vec![AccountMeta::new(counter_keypair.pubkey(), true)]
        );
        let message = Message::new(&[increment_instruction], Some(&payer.pubkey()));
        let transaction = Transaction::new(
            &[&payer, &counter_keypair],
            message,
            svm.latest_blockhash()
        );
        let result = svm.send_transaction(transaction);
        assert!(result.is_ok(), "Increment transaction should succeed");

        let logs = result.unwrap().logs;
        println!("Transaction logs:\n{:#?}", logs);
    }
}
