mod kernel;
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    metadata::Metadata,
    token::{Mint, Token, TokenAccount},
};
use anchor_lang::solana_program::{program::invoke, system_instruction};

use kernel::event_manager::EventManager;
use kernel::nft_manager::NftManager;
use kernel::ticket_manager::TicketManager;
use mpl_token_metadata::accounts::{MasterEdition, Metadata as MetadataAccount};

// Déclare l'ID du programme.
declare_id!("DTKMbemCwskuw6sTCc2aRYvC9YGHjyQEWxJ6M6C24sTF");

#[program]
pub mod nft_ticketing {
    use super::*;

    pub fn sponsor_event(
        ctx : Context<FundEvent>,
        date_of_funding : i64,
        amount: u64,
        sponsor_name: String,  // New parameter for sponsor name
        nft_name: String,   // Add NFT name as input
        nft_symbol: String, // Add NFT symbol as input
        nft_uri: String,    // Add NFT metadata URI as input
        )->Result<()>{
        let funding = &mut ctx.accounts.funding;
        let event = &ctx.accounts.event;

        // Verify that the organizer is correct
        if ctx.accounts.organizer.key() != event.organizer {
            return Err(CustomError::CreateTicketInvalidOrganizer.into());
        }

        funding.event = event.key(); // Join the funding to the event.
        funding.amount = amount; // Set the funding amount.
        funding.date_of_sponsor = date_of_funding; // Set the date of funding.
        funding.sponsor = *ctx.accounts.sponsor.key; // Set the sponsor's public key.
        funding.sponsor_name = sponsor_name;  // Store the sponsor's name

        let lamports = amount;

        // Transfer SOL from sponsor to organizer
        // OPTIMISATION : ajouter un test pour tester le transfert de lamports.
        invoke(
            &system_instruction::transfer(
                &ctx.accounts.sponsor.key(),
                &event.organizer, // Transfer to the event organizer
                lamports,
            ),
            &[
                ctx.accounts.sponsor.to_account_info(),
                ctx.accounts.organizer.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        // Call `create_nft` and store the NFT mint
        let nft_mint = NftManager::run_create_nft(
            ctx.accounts.nft_context(),
            nft_name,
            nft_symbol,
            nft_uri,
        )?;

        funding.nft_mint = Some(nft_mint);

        Ok(())

    }

    // Instruction permettant de créer un événement.
    pub fn create_event(
        ctx: Context<CreateEvent>,
        title: String,
        description: String,
        date: i64,
        location: String,
        ticket_price: u64,
    ) -> Result<()> {
        EventManager::run_create_event(ctx, title, description, date, location, ticket_price)
    }

    // Instruction permettant de créer un ticket pour un événement.
    pub fn buy_ticket(ctx: Context<BuyTicket>, date_of_purchase: i64) -> Result<()> {
        TicketManager::run_buy_ticket(ctx, date_of_purchase)
    }

    // Déclare la fonction create_nft et appelle la fonction externalisée
    pub fn create_nft(
        ctx: Context<CreateNft>,
        name: String,
        symbol: String,
        uri: String,
    ) -> Result<()> {
        NftManager::run_create_nft(ctx, name, symbol, uri)
    }
}

// Contexte de l'instruction permettant de créer un événement.
#[derive(Accounts)]
pub struct CreateEvent<'info> {
    // event: web3.Keypair.generate()
    // Initialise le compte de l'événement, en spécifiant le payeur et l'espace nécessaire.
    #[account(init, payer = organizer, space = 8 + 32 + 4 + 100 + 4 + 256 + 8 + 4 + 100 + 8)]
    pub event: Account<'info, Event>,
    #[account(mut)]
    pub organizer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

// Contexte de l'instruction permettant de créer un ticker pour un X événement.
#[derive(Accounts)]
pub struct BuyTicket<'info> {
    // ticket: web3.Keypair.generate()
    // Initialise le compte du ticket, en spécifiant le payeur et l'espace nécessaire.
    #[account(init, payer = owner, space = 8 + 32 + 8 + 8 + 32 + 33)]
    pub ticket: Account<'info, Ticket>,
    // event: pour joindre le ticket à un X event.
    pub event: Account<'info, Event>,
    #[account(mut)]
    pub owner: Signer<'info>,
    /// CHECK: The organizer is not checked because it's part of the event account.
    #[account(mut)]
    pub organizer: AccountInfo<'info>, // Ajouté spécialement afin de pouvoir effectuer le transfert des lamports.
    pub system_program: Program<'info, System>,
}

// Structure de comptes nécessaire pour l'instruction `create_nft`.
#[derive(Accounts)]
pub struct CreateNft<'info> {
    // Compte du signataire (celui qui initialise la transaction). Ce compte doit être mutable et signer la transaction.
    #[account(mut, signer)]
    pub signer: Signer<'info>,

    // Compte représentant la monnaie à frapper. Ce compte est initialisé ici.
    // Le PDA est à la fois l'adresse du compte de la monnaie et l'autorité de la monnaie.
    // La monnaie aura 0 décimales (typique pour les NFTs), avec l'autorité et l'autorité de gel définies comme le signataire.
    #[account(
        init,
        payer = signer,
        mint::decimals = 0,
        mint::authority = signer.key(),
        mint::freeze_authority = signer.key(),
    )]
    pub mint: Account<'info, Mint>,

    // Compte représentant le compte associé au token. Ce compte est initialisé si nécessaire.
    // Le compte est initialisé avec la monnaie et l'autorité définies comme le signataire.
    #[account(
        init_if_needed,
        payer = signer,
        associated_token::mint = mint,
        associated_token::authority = signer
    )]
    pub associated_token_account: Account<'info, TokenAccount>,

    // Compte de métadonnées pour le NFT. Ce compte est vérifié par son adresse.
    // "find_pda" est utilisé pour trouver le PDA du compte de métadonnées basé sur la clé publique de la monnaie.
    // Nous utilisons `AccountInfo` car nous savons que cette vérification est sécurisée en raison de l'utilisation de l'adresse calculée.
    // Le compte de métadonnées contient toutes les informations relatives au NFT, telles que son nom, son symbole,
    // son URI de métadonnées (lien vers des informations détaillées sur le NFT,
    // souvent un fichier JSON hébergé quelque part), et d'autres informations supplémentaires.
    /// CHECK: The metadata_account is derived from the mint and its PDA is checked, so it's safe to use.
    #[account(
        mut,
        address = MetadataAccount::find_pda(&mint.key()).0,
    )]
    pub metadata_account: AccountInfo<'info>,

    // Compte de master edition pour le NFT. Ce compte est vérifié par son adresse.
    // "find_pda" est utilisé pour trouver le PDA du compte de master edition basé sur la clé publique de la monnaie.
    // Nous utilisons `AccountInfo` car nous savons que cette vérification est sécurisée en raison de l'utilisation de l'adresse calculée.
    // Le compte de master edition est utilisé pour représenter une édition unique du NFT.
    // Cela est particulièrement utile lorsque vous avez des éditions limitées ou des copies numérotées d'un même NFT.
    /// CHECK: The master_edition_account is derived from the mint and its PDA is checked, so it's safe to use.
    #[account(
        mut,
        address = MasterEdition::find_pda(&mint.key()).0,
    )]
    pub master_edition_account: AccountInfo<'info>,

    // Programme de token SPL, nécessaire pour les appels CPI qui manipulent les tokens SPL.
    pub token_program: Program<'info, Token>,

    // Programme de token associé SPL, nécessaire pour initialiser les comptes de token associés si nécessaire.
    pub associated_token_program: Program<'info, AssociatedToken>,

    // Programme de métadonnées de token, nécessaire pour les appels CPI qui manipulent les métadonnées de token.
    pub token_metadata_program: Program<'info, Metadata>,

    // Programme système de Solana, nécessaire pour les appels CPI qui effectuent des actions telles que la création de comptes.
    pub system_program: Program<'info, System>,

    // Sysvar représentant les frais de location (rent) de Solana. Utilisé pour vérifier et payer les frais de location lors de la création de comptes.
    pub rent: Sysvar<'info, Rent>,

    // Pour joindre le NFT au ticket.
    // ticket: pour joindre le NFT à un X ticket.
    // (un ticket peut optionnellement avoir un NFT, un NFT doit être joint à un ticket).
    // Compte de ticket. Ce compte doit exister et être mutable.
    // Doit être mutable, car on joint le NFT au ticket (on ne peut pas faire le contraire : joindre le ticket au ticket).
    #[account(mut)]
    pub ticket: Account<'info, Ticket>,
}

// Structure pour stocker les informations de l'événement.
#[account]
pub struct Event {
    pub title: String,
    pub description: String,
    pub date: i64,
    pub location: String,
    pub ticket_price: u64, // Valeur en Lamports. PS: 1 SOL = 10^9 Lamports. 1 SOL = 1,000,000,000 Lamports (9 zéros).
    pub organizer: Pubkey, // Clé publique de l'organisateur de l'événement.
}

#[account]
pub struct Ticket {
    pub event: Pubkey,
    pub price: u64,
    pub date_of_purchase: i64,
    pub owner: Pubkey,
    // nft_mint: pour éventuellement joindre un NFT au ticket.
    // (un ticket peut optionnellement avoir un NFT, un NFT doit être joint à un ticket).
    pub nft_mint: Option<Pubkey>,
}

#[account]
#[derive(Default)]
pub struct Funding {
    pub event: Pubkey,
    pub amount: u64,
    pub date_of_sponsor: i64,
    pub sponsor: Pubkey,
    pub sponsor_name: String, // New field to store the sponsor's name.
    // nft_mint: pour éventuellement joindre un NFT au ticket.
    // (un ticket peut optionnellement avoir un NFT, un NFT doit être joint à un ticket).
    pub nft_mint: Option<Pubkey>,
}

#[derive(Accounts)]
pub struct FundEvent<'info> {
    // ticket: web3.Keypair.generate()
    // Initialize the space needed for the Account (Fund)
    #[account(init, payer = sponsor, space = 8 + 32 + 8 + 32 + 4 + 100 + 8)] // Adjust space
    pub funding: Account<'info, Funding>,
    // Event that will be sponsored
    #[account(mut)] // Ensure the event account can be modified
    pub event: Account<'info, Event>,
    // Sponsor who is funding the event
    #[account(mut)]
    pub sponsor: Signer<'info>,
    /// CHECK: The organizer is not checked because it's part of the event account.
    #[account(mut)]
    pub organizer: AccountInfo<'info>, // Needed to facilitate the transfer of lamports
    // System program for transaction execution
    pub system_program: Program<'info, System>,
}

#[error_code]
pub enum CustomError {
    #[msg("L'organisateur fourni ne correspond pas à l'organisateur de l'événement.")]
    CreateTicketInvalidOrganizer,
    #[msg("Le signataire fourni ne correspond pas au propriétaire du ticket.")]
    CreateNftUnauthorizedSigner,
    #[msg("Le ticket a déjà un NFT.")]
    TicketAlreadyHasNft,
}
