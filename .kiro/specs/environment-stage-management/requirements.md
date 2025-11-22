# Requirements Document

## Introduction

This feature extends the existing environment management system to support multiple deployment stages (dev, qa, preprod, prod) across different runtime environments (local, docker, orbstack, k8s). Currently, the system supports basic .env file loading for runtime environments, but lacks the ability to distinguish between deployment stages. This enhancement will allow users to manage stage-specific configurations (e.g., .env.dev, .env.qa, .env.prod) and automatically or manually select the appropriate environment file based on the deployment stage.

## Glossary

- **Runtime Environment**: The execution context where an app runs (local, docker, orbstack, k8s)
- **Deployment Stage**: The lifecycle stage of the application (dev, qa, preprod, prod)
- **Environment File**: A .env file containing key-value pairs for environment variables
- **Stage-Specific Environment File**: An environment file with a stage suffix (e.g., .env.dev, .env.qa)
- **devcli System**: The CLI tool that manages application lifecycle and configuration
- **App Configuration**: The JSON structure in ~/.devcli/config.json defining app settings
- **Auto-Detection**: Automatic discovery of available environment files in the app directory
- **Stage Selection**: The process of choosing which deployment stage to use for an app
- **Environment Variable Loading**: Reading and parsing .env files to extract key-value pairs
- **Priority Order**: The sequence in which environment files are searched and loaded

## Requirements

### Requirement 1

**User Story:** As a developer, I want to define multiple deployment stages for my applications, so that I can manage different configurations for dev, qa, preprod, and prod environments.

#### Acceptance Criteria

1. WHEN the devcli System loads an App Configuration, THE devcli System SHALL support an optional "stage" field in the App Configuration that accepts values "dev", "qa", "preprod", or "prod"
2. WHEN the devcli System validates an App Configuration with a "stage" field, THE devcli System SHALL reject configurations with stage values other than "dev", "qa", "preprod", or "prod"
3. WHEN the devcli System loads an App Configuration without a "stage" field, THE devcli System SHALL treat the app as having no specific deployment stage
4. THE devcli System SHALL persist the stage configuration in the ~/.devcli/config.json file
5. WHEN the devcli System displays app information, THE devcli System SHALL include the deployment stage if configured

### Requirement 2

**User Story:** As a developer, I want the system to automatically detect stage-specific environment files, so that I don't have to manually configure file paths for each stage.

#### Acceptance Criteria

1. WHEN the devcli System searches for Environment Files in an app directory, THE devcli System SHALL detect files matching the patterns ".env", ".env.dev", ".env.qa", ".env.preprod", and ".env.prod"
2. WHEN an App Configuration specifies a Deployment Stage, THE devcli System SHALL prioritize the Stage-Specific Environment File matching that stage over the base .env file
3. WHEN the devcli System searches for Environment Files with a dockerfile_path configured, THE devcli System SHALL search in the Dockerfile directory first, then fall back to the app root directory
4. WHEN no Stage-Specific Environment File exists for the configured stage, THE devcli System SHALL fall back to the base .env file
5. WHEN neither a Stage-Specific Environment File nor a base .env file exists, THE devcli System SHALL proceed without loading environment variables

### Requirement 3

**User Story:** As a developer, I want to specify which deployment stage to use when starting an app, so that I can override the default stage configuration for testing purposes.

#### Acceptance Criteria

1. WHEN the devcli System executes a start command, THE devcli System SHALL accept an optional "--stage" flag with values "dev", "qa", "preprod", or "prod"
2. WHEN the "--stage" flag is provided, THE devcli System SHALL use the specified stage instead of the stage from App Configuration
3. WHEN the "--stage" flag is provided with an invalid value, THE devcli System SHALL display an error message listing valid stage values and terminate
4. WHEN the devcli System loads Environment Variables with a stage override, THE devcli System SHALL use the Stage-Specific Environment File for the overridden stage
5. WHEN the devcli System completes a start command with a stage override, THE devcli System SHALL not persist the override to the App Configuration

### Requirement 4

**User Story:** As a developer, I want to configure the default deployment stage for an app through the config command, so that I can set persistent stage preferences without editing JSON files manually.

#### Acceptance Criteria

1. WHEN the devcli System executes the config edit command, THE devcli System SHALL provide an option to set the deployment stage
2. WHEN a user selects the stage configuration option, THE devcli System SHALL display available stages: "dev", "qa", "preprod", "prod", and "none"
3. WHEN a user selects "none" for the stage, THE devcli System SHALL remove the stage field from the App Configuration
4. WHEN a user selects a specific stage, THE devcli System SHALL update the App Configuration with the selected stage
5. WHEN the devcli System saves the updated App Configuration, THE devcli System SHALL validate that the stage value is one of the allowed values

### Requirement 5

**User Story:** As a developer, I want the system to show me which environment file will be used for each runtime environment and stage combination, so that I can verify my configuration is correct.

#### Acceptance Criteria

1. WHEN the devcli System executes the config list command for an app, THE devcli System SHALL display the configured deployment stage if present
2. WHEN the devcli System displays app configuration details, THE devcli System SHALL show which Environment File would be loaded for each Runtime Environment
3. WHEN multiple Stage-Specific Environment Files exist, THE devcli System SHALL indicate which file has priority based on the current stage configuration
4. WHEN no Environment File exists for a Runtime Environment and stage combination, THE devcli System SHALL display "No environment file" for that combination
5. WHEN the devcli System displays environment file information, THE devcli System SHALL show the relative path from the app root directory

### Requirement 6

**User Story:** As a developer, I want environment variables from stage-specific files to be loaded when running apps in docker and orbstack environments, so that my containerized apps use the correct configuration.

#### Acceptance Criteria

1. WHEN the devcli System starts an app in the docker Runtime Environment, THE devcli System SHALL load Environment Variables from the appropriate Stage-Specific Environment File based on the configured or overridden stage
2. WHEN the devcli System starts an app in the orbstack Runtime Environment, THE devcli System SHALL load Environment Variables from the appropriate Stage-Specific Environment File based on the configured or overridden stage
3. WHEN the devcli System loads Environment Variables for docker, THE devcli System SHALL pass variables using the --env-file flag if supported
4. WHEN the devcli System loads Environment Variables for orbstack, THE devcli System SHALL pass variables using individual -e flags for each key-value pair
5. WHEN Environment Variable Loading fails, THE devcli System SHALL display an error message indicating which file failed to load and terminate the start operation

### Requirement 7

**User Story:** As a developer, I want the auto-add command to detect and configure stage-specific environment files, so that new apps are automatically configured with the appropriate stage settings.

#### Acceptance Criteria

1. WHEN the devcli System executes the auto-add command for an app, THE devcli System SHALL scan the app directory for Stage-Specific Environment Files
2. WHEN the devcli System detects multiple Stage-Specific Environment Files during auto-add, THE devcli System SHALL prompt the user to select a default stage
3. WHEN the user selects a default stage during auto-add, THE devcli System SHALL add the stage field to the App Configuration
4. WHEN only a base .env file exists during auto-add, THE devcli System SHALL not set a stage field in the App Configuration
5. WHEN the devcli System completes auto-add with stage configuration, THE devcli System SHALL display which Stage-Specific Environment Files were detected

### Requirement 8

**User Story:** As a developer, I want the system to maintain backward compatibility with existing configurations, so that my current apps continue to work without modification.

#### Acceptance Criteria

1. WHEN the devcli System loads an App Configuration without a stage field, THE devcli System SHALL use the base .env file as before
2. WHEN the devcli System processes Environment Variable Loading for apps without stage configuration, THE devcli System SHALL follow the existing Priority Order (Dockerfile directory .env, then root .env)
3. WHEN the devcli System encounters an App Configuration created before this feature, THE devcli System SHALL load and execute the app without errors
4. WHEN the devcli System validates an App Configuration, THE devcli System SHALL treat the stage field as optional
5. WHEN the devcli System serializes an App Configuration without a stage, THE devcli System SHALL omit the stage field from the JSON output

### Requirement 9

**User Story:** As a developer, I want clear error messages when stage-specific environment files are missing, so that I can quickly identify and fix configuration issues.

#### Acceptance Criteria

1. WHEN the devcli System attempts to load a Stage-Specific Environment File that does not exist, THE devcli System SHALL display a warning message indicating the missing file and the fallback behavior
2. WHEN the devcli System starts an app with a configured stage but no matching Stage-Specific Environment File, THE devcli System SHALL display which file was expected and which file is being used instead
3. WHEN the devcli System encounters an invalid stage value in App Configuration, THE devcli System SHALL display an error message listing valid stage values
4. WHEN Environment Variable Loading fails due to file read errors, THE devcli System SHALL display the file path and the specific error reason
5. WHEN the devcli System displays stage-related errors, THE devcli System SHALL include suggestions for resolving the issue

### Requirement 10

**User Story:** As a developer, I want stage information to be visible in the TUI monitor view, so that I can see which stage each running app is using.

#### Acceptance Criteria

1. WHEN the devcli System displays an app in the TUI monitor view, THE devcli System SHALL show the deployment stage if configured
2. WHEN an app is started with a stage override, THE devcli System SHALL display the overridden stage in the TUI monitor view
3. WHEN the devcli System displays stage information in the TUI, THE devcli System SHALL use a distinct visual indicator (e.g., color or label) to differentiate stages
4. WHEN an app has no configured stage, THE devcli System SHALL display "No stage" or omit the stage field in the TUI monitor view
5. WHEN the devcli System updates the TUI display, THE devcli System SHALL refresh stage information if the configuration changes
