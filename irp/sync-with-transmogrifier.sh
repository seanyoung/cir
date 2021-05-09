#!/bin/bash

IRPDIR="$( cd "$( dirname "$0" )" && pwd )"
TRANSMOGRIFIER_DIR=$1
if [ -z "${TRANSMOGRIFIER_DIR}" -o \! -d "${TRANSMOGRIFIER_DIR}" ]; then
	echo "Usage: $0 TRANSMOGRIFIER_DIR"
	echo
	echo "TRANSMOGRIFIER_DIR must be set to a checked out IrpTransmogrifier, e.g.:"
	echo "git checkout https://github.com/bengtmartensson/IrpTransmogrifier/"
	exit 1
fi

# Build it so we can have test data
cd ${TRANSMOGRIFIER_DIR}
mvn compile
mvn package

# Update our IrpProtocols
cp -a src/main/resources/IrpProtocols.xml ${IRPDIR}/

export PATH=${TRANSMOGRIFIER_DIR}/target:$PATH

# Generate out test data
cd ${IRPDIR}/generate_test_data/
cargo run

# Now test our encoder
cd ${IRPDIR}
cargo test
