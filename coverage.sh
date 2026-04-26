#!/bin/bash
################################################################################
#
#    Copyright (c) 2026.
#    Haixing Hu, Qubit Co. Ltd.
#
#    All rights reserved.
#
################################################################################
#
# Code coverage testing script
# Uses cargo-llvm-cov to generate code coverage reports
#

set -euo pipefail

MIN_FUNCTION_COVERAGE=100
MIN_LINE_COVERAGE=98
MIN_REGION_COVERAGE=98
COVERAGE_JSON_PATH="target/llvm-cov/coverage.json"

check_json_coverage_thresholds() {
    local report_path="$1"

    if ! command -v jq > /dev/null 2>&1; then
        echo "❌ Error: jq is required to validate JSON coverage thresholds"
        echo "Install jq with your system package manager, then rerun this script."
        exit 1
    fi

    local source_file_count
    source_file_count=$(jq \
        --arg root "$CURRENT_CRATE_DIR/" \
        '[.data[0].files[] | select(.filename | startswith($root + "src/"))] | length' \
        "$report_path")
    if [ "$source_file_count" -eq 0 ]; then
        echo "❌ Error: no src/ files found in coverage report"
        exit 1
    fi

    local failures
    failures=$(jq -r \
        --arg root "$CURRENT_CRATE_DIR/" \
        --argjson min_lines "$MIN_LINE_COVERAGE" \
        --argjson min_regions "$MIN_REGION_COVERAGE" \
        '.data[0].files[]
        | select(.filename | startswith($root + "src/"))
        | {
            file: (.filename | ltrimstr($root)),
            functions_total: .summary.functions.count,
            functions_covered: .summary.functions.covered,
            lines_percent: .summary.lines.percent,
            regions_percent: .summary.regions.percent
        }
        | select(
            (.functions_covered != .functions_total)
            or (.lines_percent <= $min_lines)
            or (.regions_percent <= $min_regions)
        )
        | "\(.file): functions=\(.functions_covered)/\(.functions_total), lines=\(.lines_percent)%, regions=\(.regions_percent)%"' \
        "$report_path")

    if [ -n "$failures" ]; then
        echo "❌ Coverage threshold check failed"
        echo "Required per src/ file: functions = ${MIN_FUNCTION_COVERAGE}%, lines > ${MIN_LINE_COVERAGE}%, regions > ${MIN_REGION_COVERAGE}%"
        echo "$failures"
        exit 1
    fi

    echo "✅ Coverage thresholds passed"
    echo "   Per src/ file: functions = ${MIN_FUNCTION_COVERAGE}%, lines > ${MIN_LINE_COVERAGE}%, regions > ${MIN_REGION_COVERAGE}%"
}

echo "🔍 Starting code coverage testing..."

# Switch to project directory
cd "$(dirname "$0")"

# Detect package name from Cargo.toml
if [ -f "Cargo.toml" ]; then
    PACKAGE_NAME=$(grep "^name = " Cargo.toml | head -n 1 | sed 's/name = "\(.*\)"/\1/')
    echo "📦 Detected package: $PACKAGE_NAME"
else
    echo "❌ Error: Cargo.toml not found in current directory"
    exit 1
fi

# Get current directory absolute path to filter coverage
CURRENT_CRATE_DIR=$(pwd)
echo "📁 Coverage will only include files in: $CURRENT_CRATE_DIR"

# Build regex pattern to exclude third-party code and other workspace members
CURRENT_CRATE_NAME=$(basename "$CURRENT_CRATE_DIR")
WORKSPACE_ROOT=$(cd "$(dirname "$0")/.." && pwd)

# Create list of other workspace crates to exclude (any sibling directory)
OTHER_CRATES=""
for crate_dir in "$WORKSPACE_ROOT"/*/; do
    [ -d "$crate_dir" ] || continue
    crate_name=$(basename "$crate_dir")
    if [ "$crate_name" != "$CURRENT_CRATE_NAME" ]; then
        if [ -z "$OTHER_CRATES" ]; then
            OTHER_CRATES="$crate_name"
        else
            OTHER_CRATES="$OTHER_CRATES|$crate_name"
        fi
    fi
done

# Exclude: cargo registry, rustup, and other workspace crates
# Using simple alternation for clarity
EXCLUDE_PATTERN="(\.cargo/registry|\.rustup/|/($OTHER_CRATES)/)"
echo "🚫 Excluding: .cargo/registry, .rustup, and other workspace members"

# Parse arguments, check if cleanup is needed
CLEAN_FLAG=""
FORMAT_ARG=""

for arg in "$@"; do
    case "$arg" in
        --clean)
            CLEAN_FLAG="yes"
            ;;
        *)
            FORMAT_ARG="$arg"
            ;;
    esac
done

# Default format is html
FORMAT_ARG="${FORMAT_ARG:-html}"

# If --clean option is specified, clean old data
if [ "$CLEAN_FLAG" = "yes" ]; then
    echo "🧹 Cleaning old coverage data..."
    cargo llvm-cov clean
else
    echo "ℹ️  Using cached build (use --clean option if you need to clean cache)"
fi

# cargo-llvm-cov does not create parent directories for --json/--lcov/--cobertura outputs
mkdir -p target/llvm-cov

# Run tests and generate coverage reports
case "$FORMAT_ARG" in
    html)
        echo "📊 Generating HTML format coverage report..."
        cargo llvm-cov --package "$PACKAGE_NAME" --html --open \
            --ignore-filename-regex "$EXCLUDE_PATTERN"
        echo "✅ HTML report generated and opened in browser"
        echo "   Report location: target/llvm-cov/html/index.html"
        ;;

    text)
        echo "📊 Generating text format coverage report..."
        cargo llvm-cov --package "$PACKAGE_NAME" \
            --ignore-filename-regex "$EXCLUDE_PATTERN"
        ;;

    lcov)
        echo "📊 Generating LCOV format coverage report..."
        cargo llvm-cov --package "$PACKAGE_NAME" --lcov --output-path target/llvm-cov/lcov.info \
            --ignore-filename-regex "$EXCLUDE_PATTERN"
        echo "✅ LCOV report generated"
        echo "   Report location: target/llvm-cov/lcov.info"
        ;;

    json)
        echo "📊 Generating JSON format coverage report..."
        cargo llvm-cov --package "$PACKAGE_NAME" --json --output-path "$COVERAGE_JSON_PATH" \
            --ignore-filename-regex "$EXCLUDE_PATTERN"
        echo "✅ JSON report generated"
        echo "   Report location: $COVERAGE_JSON_PATH"
        check_json_coverage_thresholds "$COVERAGE_JSON_PATH"
        ;;

    cobertura)
        echo "📊 Generating Cobertura XML format coverage report..."
        cargo llvm-cov --package "$PACKAGE_NAME" --cobertura --output-path target/llvm-cov/cobertura.xml \
            --ignore-filename-regex "$EXCLUDE_PATTERN"
        echo "✅ Cobertura report generated"
        echo "   Report location: target/llvm-cov/cobertura.xml"
        ;;

    all)
        echo "📊 Generating all format coverage reports..."

        # HTML
        echo "  - Generating HTML report..."
        cargo llvm-cov --package "$PACKAGE_NAME" --html \
            --ignore-filename-regex "$EXCLUDE_PATTERN"

        # LCOV
        echo "  - Generating LCOV report..."
        cargo llvm-cov --package "$PACKAGE_NAME" --lcov --output-path target/llvm-cov/lcov.info \
            --ignore-filename-regex "$EXCLUDE_PATTERN"

        # JSON
        echo "  - Generating JSON report..."
        cargo llvm-cov --package "$PACKAGE_NAME" --json --output-path "$COVERAGE_JSON_PATH" \
            --ignore-filename-regex "$EXCLUDE_PATTERN"

        # Cobertura
        echo "  - Generating Cobertura XML report..."
        cargo llvm-cov --package "$PACKAGE_NAME" --cobertura --output-path target/llvm-cov/cobertura.xml \
            --ignore-filename-regex "$EXCLUDE_PATTERN"

        echo "✅ All format reports generated"
        echo "   HTML:      target/llvm-cov/html/index.html"
        echo "   LCOV:      target/llvm-cov/lcov.info"
        echo "   JSON:      $COVERAGE_JSON_PATH"
        echo "   Cobertura: target/llvm-cov/cobertura.xml"
        check_json_coverage_thresholds "$COVERAGE_JSON_PATH"
        ;;

    help|--help|-h)
        echo "Usage: ./coverage.sh [format] [options]"
        echo ""
        echo "Format options:"
        echo "  html       Generate HTML report and open in browser (default)"
        echo "  text       Output text format report to terminal"
        echo "  lcov       Generate LCOV format report"
        echo "  json       Generate JSON report and enforce coverage thresholds"
        echo "  cobertura  Generate Cobertura XML format report"
        echo "  all        Generate all format reports"
        echo "  help       Show this help information"
        echo ""
        echo "Options:"
        echo "  --clean    Clean old coverage data and build cache before running"
        echo "             By default, cached builds are used to speed up compilation"
        echo ""
        echo "Requirements:"
        echo "  jq         Required for json/all coverage threshold validation"
        echo ""
        echo "Performance tips:"
        echo "  • First run will be slower (needs to compile all dependencies)"
        echo "  • Subsequent runs will be much faster (using cache)"
        echo "  • Only use --clean when dependencies are updated or major code changes"
        echo ""
        echo "Examples:"
        echo "  ./coverage.sh              # Generate HTML report (using cache)"
        echo "  ./coverage.sh text         # Output text report (using cache)"
        echo "  ./coverage.sh --clean      # Clean then generate HTML report"
        echo "  ./coverage.sh html --clean # Clean then generate HTML report"
        echo "  ./coverage.sh all --clean  # Clean then generate all formats"
        exit 0
        ;;

    *)
        echo "❌ Error: Unknown format '$FORMAT_ARG'"
        echo "Run './coverage.sh help' to see available options"
        exit 1
        ;;
esac

echo "✅ Code coverage testing completed!"
