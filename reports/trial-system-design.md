# Trial System Design for devcli

## Overview

Implement a 32-day commercial trial tracking system as specified by the PolyForm Noncommercial License 1.0.0.

## License Requirements

From PolyForm Noncommercial License:
- **Noncommercial use**: Unlimited, free forever
- **Commercial evaluation**: 32 consecutive calendar days
- **After trial**: Must purchase license or stop commercial use

## Design Goals

1. **Non-intrusive**: Don't block personal/noncommercial use
2. **Transparent**: Users should know their trial status
3. **Honor system**: Trust-based with reminders (industry standard)
4. **Privacy-respecting**: Store data locally only
5. **Optional**: Can be disabled with environment variable

## Architecture

### Storage Location

```
~/.devcli/
├── config.json           # Existing config
├── trial.json            # NEW: Trial tracking data
└── logs/                 # Existing logs
```

### Trial Data Structure

**File**: `~/.devcli/trial.json`

```json
{
  "version": "1",
  "first_use": "2024-02-07T10:30:00Z",
  "commercial_trial_start": "2024-02-10T14:22:00Z",
  "license_key": null,
  "acknowledged_noncommercial": false,
  "last_check": "2024-02-15T09:15:00Z"
}
```

**Fields:**
- `version`: Schema version for future migrations
- `first_use`: Timestamp of first ever use (any purpose)
- `commercial_trial_start`: Timestamp when commercial use started (null if never)
- `license_key`: Commercial license key (null if unlicensed)
- `acknowledged_noncommercial`: User confirmed noncommercial use
- `last_check`: Last time we checked/reminded about trial

### Use Detection Strategy

**Option 1: Honor System (Recommended)**
- Ask user on first run: "Commercial or noncommercial use?"
- Store their answer
- Show reminders based on their selection
- Trust the user (standard industry practice)

**Option 2: Heuristic Detection**
- Detect company email domains in git config
- Check for corporate environment variables
- Look for company-specific paths
- ⚠️ Complex, error-prone, privacy concerns

**Option 3: Environment Variable**
- User sets `DEVCLI_COMMERCIAL=true` or `DEVCLI_NONCOMMERCIAL=true`
- Explicit declaration
- Good for CI/CD environments

**Recommended: Combination of Option 1 + 3**

## Implementation Plan

### Phase 1: Data Storage Module

**New file**: `devcli-core/src/trial/mod.rs`

```rust
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialData {
    pub version: String,
    pub first_use: DateTime<Utc>,
    pub commercial_trial_start: Option<DateTime<Utc>>,
    pub license_key: Option<String>,
    pub acknowledged_noncommercial: bool,
    pub last_check: DateTime<Utc>,
}

impl TrialData {
    pub fn new() -> Self {
        Self {
            version: "1".to_string(),
            first_use: Utc::now(),
            commercial_trial_start: None,
            license_key: None,
            acknowledged_noncommercial: false,
            last_check: Utc::now(),
        }
    }

    pub fn trial_days_remaining(&self) -> Option<i64> {
        if let Some(start) = self.commercial_trial_start {
            let elapsed = Utc::now().signed_duration_since(start);
            let remaining = 32 - elapsed.num_days();
            Some(remaining.max(0))
        } else {
            None // Trial hasn't started
        }
    }

    pub fn is_trial_expired(&self) -> bool {
        if let Some(remaining) = self.trial_days_remaining() {
            remaining <= 0
        } else {
            false // No trial started = not expired
        }
    }

    pub fn has_valid_license(&self) -> bool {
        self.license_key.is_some()
    }

    pub fn is_compliant(&self) -> bool {
        // Compliant if:
        // - Has valid license, OR
        // - Acknowledged noncommercial use, OR
        // - Trial not expired
        self.has_valid_license()
            || self.acknowledged_noncommercial
            || !self.is_trial_expired()
    }
}

pub struct TrialManager {
    trial_path: PathBuf,
}

impl TrialManager {
    pub fn new() -> Result<Self> {
        let home = dirs::home_dir().context("No home directory")?;
        let trial_path = home.join(".devcli").join("trial.json");
        Ok(Self { trial_path })
    }

    pub fn load_or_create(&self) -> Result<TrialData> {
        if self.trial_path.exists() {
            let content = std::fs::read_to_string(&self.trial_path)?;
            let data: TrialData = serde_json::from_str(&content)?;
            Ok(data)
        } else {
            let data = TrialData::new();
            self.save(&data)?;
            Ok(data)
        }
    }

    pub fn save(&self, data: &TrialData) -> Result<()> {
        let parent = self.trial_path.parent().context("No parent dir")?;
        std::fs::create_dir_all(parent)?;
        let json = serde_json::to_string_pretty(data)?;
        std::fs::write(&self.trial_path, json)?;
        Ok(())
    }

    pub fn start_commercial_trial(&self) -> Result<TrialData> {
        let mut data = self.load_or_create()?;
        if data.commercial_trial_start.is_none() {
            data.commercial_trial_start = Some(Utc::now());
            self.save(&data)?;
        }
        Ok(data)
    }

    pub fn acknowledge_noncommercial(&self) -> Result<TrialData> {
        let mut data = self.load_or_create()?;
        data.acknowledged_noncommercial = true;
        self.save(&data)?;
        Ok(data)
    }

    pub fn set_license_key(&self, key: String) -> Result<TrialData> {
        let mut data = self.load_or_create()?;
        data.license_key = Some(key);
        self.save(&data)?;
        Ok(data)
    }
}
```

### Phase 2: First-Run Experience

**New file**: `devcli-core/src/trial/first_run.rs`

```rust
use super::{TrialData, TrialManager};
use inquire::Select;

pub fn handle_first_run() -> Result<TrialData> {
    let manager = TrialManager::new()?;
    let data = manager.load_or_create()?;

    // Skip if already configured
    if data.commercial_trial_start.is_some() || data.acknowledged_noncommercial {
        return Ok(data);
    }

    // Skip if license already provided
    if data.has_valid_license() {
        return Ok(data);
    }

    // Check environment variable override
    if let Ok(val) = std::env::var("DEVCLI_COMMERCIAL") {
        if val == "true" {
            return manager.start_commercial_trial();
        }
    }
    if let Ok(val) = std::env::var("DEVCLI_NONCOMMERCIAL") {
        if val == "true" {
            return manager.acknowledge_noncommercial();
        }
    }

    // Ask user (only if interactive terminal)
    if atty::is(atty::Stream::Stdin) {
        println!("\n🎉 Welcome to devcli!");
        println!("\ndevcli is licensed under PolyForm Noncommercial License 1.0.0");
        println!("Please indicate your intended use:\n");

        let options = vec![
            "Noncommercial (personal, education, non-profit, government)",
            "Commercial evaluation (32-day trial, then requires license)",
        ];

        let answer = Select::new("How will you use devcli?", options)
            .prompt()?;

        if answer.starts_with("Noncommercial") {
            println!("\n✅ Great! You can use devcli for free for noncommercial purposes.");
            println!("See LICENSE file for details.\n");
            manager.acknowledge_noncommercial()
        } else {
            println!("\n✅ Starting 32-day commercial evaluation period.");
            println!("After 32 days, you'll need to purchase a license or stop commercial use.");
            println!("Commercial licenses start at $99/year. See LICENSE-COMMERCIAL.md\n");
            manager.start_commercial_trial()
        }
    } else {
        // Non-interactive: default to noncommercial, user can override
        // with environment variable or license key
        println!("⚠️  Non-interactive terminal detected.");
        println!("Defaulting to noncommercial use. Set DEVCLI_COMMERCIAL=true for commercial use.");
        manager.acknowledge_noncommercial()
    }
}
```

### Phase 3: Trial Status Checking

**New file**: `devcli-core/src/trial/checker.rs`

```rust
use super::{TrialData, TrialManager};
use chrono::{Duration, Utc};

pub struct TrialChecker {
    manager: TrialManager,
}

impl TrialChecker {
    pub fn new() -> Result<Self> {
        Ok(Self {
            manager: TrialManager::new()?,
        })
    }

    pub fn check_and_notify(&self) -> Result<TrialData> {
        let mut data = self.manager.load_or_create()?;

        // Skip checks if licensed or noncommercial
        if data.has_valid_license() || data.acknowledged_noncommercial {
            return Ok(data);
        }

        // Check if commercial trial is active
        if let Some(start) = data.commercial_trial_start {
            let days_remaining = data.trial_days_remaining().unwrap_or(0);

            // Only notify once per day
            let last_check_age = Utc::now().signed_duration_since(data.last_check);
            if last_check_age < Duration::days(1) {
                return Ok(data);
            }

            // Update last check time
            data.last_check = Utc::now();
            self.manager.save(&data)?;

            // Notify based on days remaining
            if days_remaining == 0 {
                self.show_trial_expired_message();
            } else if days_remaining <= 7 {
                self.show_trial_warning(days_remaining);
            }
        }

        Ok(data)
    }

    fn show_trial_warning(&self, days_remaining: i64) {
        eprintln!("\n⏰ Commercial Trial Notice");
        eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        eprintln!("Your commercial evaluation period expires in {} day(s).", days_remaining);
        eprintln!("\nAfter expiration, you must either:");
        eprintln!("  • Purchase a commercial license ($99/year)");
        eprintln!("  • Stop using devcli for commercial purposes");
        eprintln!("\nCommercial licenses: devcli@delalonde.dev");
        eprintln!("See LICENSE-COMMERCIAL.md for details.\n");
    }

    fn show_trial_expired_message(&self) {
        eprintln!("\n⚠️  Commercial Trial Expired");
        eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        eprintln!("Your 32-day commercial evaluation period has ended.");
        eprintln!("\nTo continue commercial use, you must purchase a license.");
        eprintln!("Commercial licenses start at $99/year per developer.");
        eprintln!("\nContact: devcli@delalonde.dev");
        eprintln!("Details: LICENSE-COMMERCIAL.md");
        eprintln!("\nFor noncommercial use (personal, education, non-profit),");
        eprintln!("run: devcli license set-noncommercial\n");
    }
}
```

### Phase 4: CLI Commands

**New file**: `devcli-core/src/commands/license.rs`

```rust
use crate::trial::{TrialManager, TrialData};

pub async fn license_status_command() -> Result<()> {
    let manager = TrialManager::new()?;
    let data = manager.load_or_create()?;

    println!("devcli License Status");
    println!("━━━━━━━━━━━━━━━━━━━━━");
    println!("License: PolyForm Noncommercial 1.0.0");
    println!("First use: {}", data.first_use.format("%Y-%m-%d"));

    if let Some(key) = &data.license_key {
        println!("\n✅ Commercial License Active");
        println!("License key: {}****", &key[..8]);
    } else if data.acknowledged_noncommercial {
        println!("\n✅ Noncommercial Use");
        println!("Free for personal, education, non-profit, and government use");
    } else if let Some(start) = data.commercial_trial_start {
        let remaining = data.trial_days_remaining().unwrap_or(0);
        println!("\n⏰ Commercial Trial Active");
        println!("Started: {}", start.format("%Y-%m-%d"));
        if remaining > 0 {
            println!("Days remaining: {}", remaining);
        } else {
            println!("❌ Trial expired");
        }
    } else {
        println!("\n⚠️  Usage type not set");
    }

    println!("\nFor commercial licensing: devcli@delalonde.dev");
    println!("See LICENSE-COMMERCIAL.md for details");

    Ok(())
}

pub async fn license_set_noncommercial_command() -> Result<()> {
    let manager = TrialManager::new()?;
    let data = manager.acknowledge_noncommercial()?;

    println!("✅ Set to noncommercial use");
    println!("\nYou can now use devcli for free for:");
    println!("  • Personal projects and hobby use");
    println!("  • Research and education");
    println!("  • Non-profit organizations");
    println!("  • Government institutions");
    println!("\nSee LICENSE file for full terms.");

    Ok(())
}

pub async fn license_activate_command(key: String) -> Result<()> {
    // TODO: Validate license key format
    // TODO: Optional: Call license server to verify key

    let manager = TrialManager::new()?;
    let data = manager.set_license_key(key)?;

    println!("✅ Commercial license activated");
    println!("\nThank you for supporting devcli!");
    println!("Your commercial license is now active.");
    println!("\nFor support: devcli@delalonde.dev");

    Ok(())
}
```

### Phase 5: Integration into Main Commands

**Update**: `devcli/src/main.rs`

```rust
mod trial;

// Add subcommand
#[derive(Subcommand)]
enum Commands {
    // ... existing commands

    /// License and trial management
    License {
        #[command(subcommand)]
        command: LicenseCommands,
    },
}

#[derive(Subcommand)]
enum LicenseCommands {
    /// Show license status and trial information
    Status,

    /// Set usage to noncommercial (personal, education, non-profit)
    SetNoncommercial,

    /// Activate a commercial license key
    Activate {
        /// Commercial license key
        key: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // First-run experience (only on interactive commands)
    if should_check_trial(&cli.command) {
        trial::handle_first_run()?;
        trial::check_and_notify()?;
    }

    // Handle commands...
}
```

**Update key commands**: Add trial check before execution

```rust
// In start, run, monitor commands
pub async fn start_command(args: StartCommandArgs) -> Result<()> {
    // Check trial status
    let trial_checker = TrialChecker::new()?;
    trial_checker.check_and_notify()?;

    // ... existing command logic
}
```

## User Experience Flows

### Flow 1: First-Time Personal User

```
$ devcli start myapp

🎉 Welcome to devcli!

devcli is licensed under PolyForm Noncommercial License 1.0.0
Please indicate your intended use:

? How will you use devcli?
  > Noncommercial (personal, education, non-profit, government)
    Commercial evaluation (32-day trial, then requires license)

✅ Great! You can use devcli for free for noncommercial purposes.
See LICENSE file for details.

[proceeding with command...]
```

### Flow 2: First-Time Commercial User

```
$ devcli start myapp

🎉 Welcome to devcli!

? How will you use devcli?
    Noncommercial (personal, education, non-profit, government)
  > Commercial evaluation (32-day trial, then requires license)

✅ Starting 32-day commercial evaluation period.
After 32 days, you'll need to purchase a license or stop commercial use.
Commercial licenses start at $99/year. See LICENSE-COMMERCIAL.md

[proceeding with command...]
```

### Flow 3: Commercial Trial Nearing Expiration

```
$ devcli start myapp

⏰ Commercial Trial Notice
━━━━━━━━━━━━━━━━━━━━━━━━━━━
Your commercial evaluation period expires in 5 day(s).

After expiration, you must either:
  • Purchase a commercial license ($99/year)
  • Stop using devcli for commercial purposes

Commercial licenses: devcli@delalonde.dev
See LICENSE-COMMERCIAL.md for details.

[proceeding with command...]
```

### Flow 4: Check License Status

```
$ devcli license status

devcli License Status
━━━━━━━━━━━━━━━━━━━━━
License: PolyForm Noncommercial 1.0.0
First use: 2024-02-07

⏰ Commercial Trial Active
Started: 2024-02-10
Days remaining: 18

For commercial licensing: devcli@delalonde.dev
See LICENSE-COMMERCIAL.md for details
```

### Flow 5: Activate Commercial License

```
$ devcli license activate ABC123-DEF456-GHI789

✅ Commercial license activated

Thank you for supporting devcli!
Your commercial license is now active.

For support: devcli@delalonde.dev
```

## Environment Variables

```bash
# Override license detection (CI/CD, automation)
DEVCLI_COMMERCIAL=true        # Force commercial mode
DEVCLI_NONCOMMERCIAL=true     # Force noncommercial mode
DEVCLI_LICENSE_KEY=ABC123...  # Set license key
DEVCLI_SKIP_LICENSE_CHECK=true # Skip checks (for testing)
```

## Privacy & Security

### What We Store
- Timestamps (locally only, never sent anywhere)
- User's declaration (commercial vs noncommercial)
- License key (if provided)

### What We DON'T Do
- ❌ No telemetry or phone-home
- ❌ No tracking of usage patterns
- ❌ No collection of personal information
- ❌ No network requests for trial tracking
- ❌ No blocking or enforcement (honor system)

### License Key Validation (Optional Future)

```rust
// Optional: Call license server to verify key
async fn validate_license_key(key: &str) -> Result<bool> {
    // POST to license server
    // Verify key is valid and not expired
    // Cache result locally
}
```

## Implementation Checklist

- [ ] Create `devcli-core/src/trial/mod.rs`
- [ ] Create `devcli-core/src/trial/manager.rs` (TrialManager, TrialData)
- [ ] Create `devcli-core/src/trial/first_run.rs` (first-run experience)
- [ ] Create `devcli-core/src/trial/checker.rs` (trial status checking)
- [ ] Create `devcli-core/src/commands/license.rs` (CLI commands)
- [ ] Add `inquire` crate for interactive prompts
- [ ] Add `atty` crate for terminal detection
- [ ] Integrate trial check into main commands (start, run, monitor)
- [ ] Add `devcli license` subcommand with status/set-noncommercial/activate
- [ ] Add environment variable overrides
- [ ] Update documentation (README, getting-started.md)
- [ ] Add unit tests for trial calculations
- [ ] Add integration tests for first-run flow
- [ ] Test non-interactive mode (CI/CD)

## Testing Strategy

### Unit Tests
```rust
#[test]
fn test_trial_days_remaining() {
    let mut data = TrialData::new();
    data.commercial_trial_start = Some(Utc::now() - Duration::days(10));
    assert_eq!(data.trial_days_remaining(), Some(22));
}

#[test]
fn test_trial_expired() {
    let mut data = TrialData::new();
    data.commercial_trial_start = Some(Utc::now() - Duration::days(35));
    assert!(data.is_trial_expired());
}
```

### Integration Tests
- Test first-run with environment variables
- Test trial expiration notifications
- Test license activation
- Test noncommercial acknowledgment

## Future Enhancements

1. **License Server** (optional)
   - Validate license keys against server
   - Track activations
   - Revoke keys if needed

2. **Grace Period** (optional)
   - Allow 7-day grace after trial expiration
   - Show stronger warnings

3. **Team Licenses** (optional)
   - Track multiple developers under one license
   - License key includes seat count

4. **Offline Grace** (optional)
   - Allow X days offline before requiring validation
   - Cache validation results

## Notes

- This is an **honor system** - we trust users to comply
- Reminders are informational, not enforcement
- Standard practice for developer tools
- Balances user experience with license compliance
- Privacy-first: all data stored locally
