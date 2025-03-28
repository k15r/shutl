#!/bin/bash
#@description: Process files with additional options
#@arg:input - Input file
#@arg:output - Output file
#@arg:... - Additional files to process
#@flag:format - Output format [default:json, required]
#@bool:verbose - Enable verbose output

# Process the main input/output
echo "Processing $CLI_INPUT to $CLI_OUTPUT"

# Process additional files if any
if [ -n "$CLI_ADDITIONAL_ARGS" ]; then
    echo "Additional files: $CLI_ADDITIONAL_ARGS"
fi

# Use the format flag
if [ -n "$CLI_FORMAT" ]; then
    echo "Using format: $CLI_FORMAT"
fi

echo Verbose: $CLI_VERBOSE
# Handle trailing arguments (after --)
shift "$((OPTIND-1))"
if [ $# -gt 0 ]; then
    echo "Trailing arguments: $*"
fi


echo $CLI_TRAILING_ARGS
