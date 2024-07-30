#!/bin/bash
rm -rf target
anchor build
solana address -k target/deploy/lottery-keypair.json
anchor keys sync