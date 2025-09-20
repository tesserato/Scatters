<#
.SYNOPSIS
    Runs a comprehensive suite of linting and formatting checks for the Rust project.

.DESCRIPTION
    This script performs two main tasks:
    1. Checks code formatting using 'rustfmt'.
    2. Lints the code for style, correctness, and performance issues using 'clippy'.

    It is designed to be strict: all warnings are elevated to errors, causing the script to fail.
    This is ideal for ensuring high code quality in CI/CD pipelines.

    Use the -Fix switch to automatically apply formatting and some clippy suggestions.

.PARAMETER Fix
    If specified, the script will attempt to automatically fix formatting and linting issues.
    - 'cargo fmt' will be run to reformat files.
    - 'cargo clippy --fix' will be run to apply automatic suggestions.

.EXAMPLE
    # Run in check-only mode (default). Fails if any issues are found.
    .\lint.ps1

.EXAMPLE
    # Attempt to automatically fix all found issues.
    .\lint.ps1 -Fix
#>

param (
    [switch]$Fix
)

# Stop the script immediately if any command fails
$ErrorActionPreference = "Stop"

# Define colors for console output
$Green = "`e[32m"
$Yellow = "`e[33m"
$Red = "`e[91m"
$Reset = "`e[0m"
$Cyan = "`e[36m"

# --- Main Script Logic ---
try {
    Write-Host "${Cyan}Starting Rust code quality checks...${Reset}"
    
    # --- Step 1: Ensure required components are installed ---
    Write-Host "${Yellow}Checking for 'rustfmt' and 'clippy' components...${Reset}"
    rustup component add rustfmt, clippy | Out-Null
    Write-Host "${Green} -> Components are present.${Reset}"
    Write-Host "" # Newline for readability

    # --- Step 2: Check Code Formatting (rustfmt) ---
    Write-Host "${Yellow}Step 1: Checking code formatting with 'rustfmt'...${Reset}"
    if ($Fix) {
        Write-Host " -> Mode: Applying formatting fixes."
        cargo fmt
    }
    else {
        Write-Host " -> Mode: Check-only. No files will be changed."
        # The '--check' flag makes rustfmt fail if any file needs formatting.
        cargo fmt -- --check
    }
    Write-Host "${Green} -> Formatting is consistent.${Reset}"
    Write-Host ""

    # --- Step 3: Lint with Clippy ---
    Write-Host "${Yellow}Step 2: Linting with 'clippy' (strict mode)...${Reset}"
    # We use '-- -D warnings' to Deny (error on) all clippy warnings.
    # This enforces a very high standard of code quality.
    if ($Fix) {
        Write-Host " -> Mode: Applying automatic fixes."
        # The '--allow-dirty' and '--allow-staged' flags are useful when running in a git repo
        # to prevent clippy from aborting if you have uncommitted changes.
        cargo clippy --fix --allow-dirty --allow-staged -- -D warnings
    }
    else {
        Write-Host " -> Mode: Check-only. All warnings will be treated as errors."
        cargo clippy -- -D warnings
    }
    Write-Host "${Green} -> Clippy found no issues.${Reset}"
    Write-Host ""

    # --- Success ---
    Write-Host "${Green}✅ All code quality checks passed successfully!${Reset}"

}
catch {
    # This block runs if any command in the 'try' block fails
    Write-Host ""
    Write-Host "${Red}❌ Code quality checks failed. Please review the errors above.${Reset}"
    
    # Exit with a non-zero status code to signal failure to CI systems
    exit 1
}