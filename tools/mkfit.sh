#!/bin/sh

if [ ! "$1" ]; then echo "input file required"; exit 1; fi
if [ ! "$2" ]; then echo "output file required"; exit 1; fi

mkimage -f - -A riscv -O u-boot -T firmware $2 << EOF
/dts-v1/;

/ {
	description = "U-boot-spl FIT image for JH7110 VisionFive2";
	#address-cells = <2>;

	images {
		firmware {
			description = "u-boot";
			data = /incbin/("$1");
			type = "firmware";
			arch = "riscv";
			os = "u-boot";
			load = <0x0 0x40000000>;
			entry = <0x0 0x40000000>;
			compression = "none";
		};
	};

	configurations {
		default = "config-1";

		config-1 {
			description = "U-boot-spl FIT config for JH7110 VisionFive2";
			firmware = "firmware";
		};
	};
};
EOF
