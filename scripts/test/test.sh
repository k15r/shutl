#!/bin/bash
#@description: Process files with additional options
#@arg:input - Input file
#@arg:output - Output file
#@arg:... - Additional files to process
#@flag:format - Output format [default:json, required]
#@bool:verbose - Enable verbose output

# Process the main input/output
echo "Processing $SHUTL_INPUT to $SHUTL_OUTPUT"

# Process additional files if any
if [ -n "$SHUTL_ADDITIONAL_ARGS" ]; then
    echo "Additional files: $SHUTL_ADDITIONAL_ARGS"
fi

# Use the format flag
if [ -n "$SHUTL_FORMAT" ]; then
    echo "Using format: $SHUTL_FORMAT"
fi

echo Verbose: $SHUTL_VERBOSE
# Handle trailing arguments (after --)
shift "$((OPTIND-1))"
if [ $# -gt 0 ]; then
    echo "Trailing arguments: $*"
fi


echo $SHUTL_TRAILING_ARGS
