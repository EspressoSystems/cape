// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

// NOTE: DO NOT MODIFY! GENERATED BY SCRIPT VIA `cargo run --bin gen-vk-libraries --release`.
pragma solidity ^0.8.0;

import "../interfaces/IPlonkVerifier.sol";
import "./BN254.sol";

library Mint1In2Out24DepthVk {
    function getVk() internal pure returns (IPlonkVerifier.VerifyingKey memory vk) {
        assembly {
            // domain size
            mstore(vk, 16384)
            // num of public inputs
            mstore(add(vk, 0x20), 22)

            // sigma0
            mstore(
                mload(add(vk, 0x40)),
                14708041522873209202464618950611504807168696855480720848360413590326729841973
            )
            mstore(
                add(mload(add(vk, 0x40)), 0x20),
                2753391240893238116569628860982882954353792578019920766428726340611015647581
            )
            // sigma1
            mstore(
                mload(add(vk, 0x60)),
                3736215203151709462427825581991044329817961401819325086573903036518525176090
            )
            mstore(
                add(mload(add(vk, 0x60)), 0x20),
                12284473618321395163309979733066433449809233564826193169921444928840687100523
            )
            // sigma2
            mstore(
                mload(add(vk, 0x80)),
                11948153932361754444295437431688112113763465916556532032853808907007255324832
            )
            mstore(
                add(mload(add(vk, 0x80)), 0x20),
                5247166478759706764702942889858430530186042193040312355719301585036655612459
            )
            // sigma3
            mstore(
                mload(add(vk, 0xa0)),
                17184781586365391989471544204947701083939573062775992140067289916802254834188
            )
            mstore(
                add(mload(add(vk, 0xa0)), 0x20),
                1695548810031655609675397387003567906043418871571997772255611361115032629003
            )
            // sigma4
            mstore(
                mload(add(vk, 0xc0)),
                4501183465908078766709944423483386166697765379860531518789327025791827694266
            )
            mstore(
                add(mload(add(vk, 0xc0)), 0x20),
                17179919563903728314665267245084588379374464645703406635631119875332721091062
            )

            // q1
            mstore(
                mload(add(vk, 0xe0)),
                8233664603830467551407560711982259529601063264885744179029753653795440811880
            )
            mstore(
                add(mload(add(vk, 0xe0)), 0x20),
                15890473389663313484400232619457945250113260815521617218577960950923821395961
            )
            // q2
            mstore(
                mload(add(vk, 0x100)),
                14842917854453150581899781597532237976322234382964084206933989618934323526445
            )
            mstore(
                add(mload(add(vk, 0x100)), 0x20),
                16447842172982150537473552975294340243672291348134029457070764238385172728852
            )
            // q3
            mstore(
                mload(add(vk, 0x120)),
                9473551627160998361000472320259848783011643008757616507618705701015024223999
            )
            mstore(
                add(mload(add(vk, 0x120)), 0x20),
                11314416338785822922260197499038268393262643508579752114469422388580655977102
            )
            // q4
            mstore(
                mload(add(vk, 0x140)),
                3736408701597418834318726881826839552728418266216645424811344776852549712816
            )
            mstore(
                add(mload(add(vk, 0x140)), 0x20),
                9236488906535632856862877101736177223606785065252708856745807157980987984387
            )

            // qM12
            mstore(
                mload(add(vk, 0x160)),
                14102260043757883202366109964215541081299927672047603711818995797147714865094
            )
            mstore(
                add(mload(add(vk, 0x160)), 0x20),
                17534575210236353125951475539478479017023300116581894838767353256804423795888
            )
            // qM34
            mstore(
                mload(add(vk, 0x180)),
                9147214868025953364750888491087621905427748656716737534941501783669122960379
            )
            mstore(
                add(mload(add(vk, 0x180)), 0x20),
                1392401634629635498019533543932086568632128115192597982401550578444977393547
            )

            // qO
            mstore(
                mload(add(vk, 0x1a0)),
                10905264501530050014704452452494914745596183555206362825031535539577170367475
            )
            mstore(
                add(mload(add(vk, 0x1a0)), 0x20),
                17138899495046135206471329677572657240135846790961757879454458120765242310575
            )
            // qC
            mstore(
                mload(add(vk, 0x1c0)),
                16573281449079492002777383418086249227397635509941971752517637461403659421155
            )
            mstore(
                add(mload(add(vk, 0x1c0)), 0x20),
                4575446980340017635017887407539797482781705198893380506254262640090465211655
            )
            // qH1
            mstore(
                mload(add(vk, 0x1e0)),
                9089742723053765306677953175198389661353135493790082378155841294705327694917
            )
            mstore(
                add(mload(add(vk, 0x1e0)), 0x20),
                11133242012031704156289281393180107718619015102295906028702493235407386901280
            )
            // qH2
            mstore(
                mload(add(vk, 0x200)),
                10009477156249913501931891243909788618345391893663991287711709770530743764439
            )
            mstore(
                add(mload(add(vk, 0x200)), 0x20),
                2335006503907830689782212423634682006869891487153768081847010024128012642090
            )
            // qH3
            mstore(
                mload(add(vk, 0x220)),
                204582489322604335877947037789506354815242950315871800117188914050721754147
            )
            mstore(
                add(mload(add(vk, 0x220)), 0x20),
                4017254452065892946191861754786121551706223202798323858822829895419210960406
            )
            // qH4
            mstore(
                mload(add(vk, 0x240)),
                3674255676567461700605617197873932900311232245160095442299763249794134579502
            )
            mstore(
                add(mload(add(vk, 0x240)), 0x20),
                14717173916044651338237546750276495403229974586112157441016319173772835390378
            )
            // qEcc
            mstore(
                mload(add(vk, 0x260)),
                12191628753324517001666106106337946847104780287136368645491927996790130156414
            )
            mstore(
                add(mload(add(vk, 0x260)), 0x20),
                13305212653333031744208722140065322148127616384688600512629199891590396358314
            )
        }
    }
}
