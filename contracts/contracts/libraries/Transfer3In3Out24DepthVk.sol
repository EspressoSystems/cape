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

library Transfer3In3Out24DepthVk {
    function getVk() internal pure returns (IPlonkVerifier.VerifyingKey memory vk) {
        assembly {
            // domain size
            mstore(vk, 65536)
            // num of public inputs
            mstore(add(vk, 0x20), 45)

            // sigma0
            mstore(
                mload(add(vk, 0x40)),
                6745569324574292840123998773726184666805725845966057344477780763812378175216
            )
            mstore(
                add(mload(add(vk, 0x40)), 0x20),
                15674359264100532117390420549335759541287602785521062799291583384533749901741
            )
            // sigma1
            mstore(
                mload(add(vk, 0x60)),
                3882047939060472482494153851462770573213675187290765799393847015027127043523
            )
            mstore(
                add(mload(add(vk, 0x60)), 0x20),
                7630821036627726874781987389422412327209162597154025595018731571961516169947
            )
            // sigma2
            mstore(
                mload(add(vk, 0x80)),
                21225224708013383300469954369858606000505504678178518510917526718672976749965
            )
            mstore(
                add(mload(add(vk, 0x80)), 0x20),
                16365929799382131762072204211493784381011606251973921052275294268694891754790
            )
            // sigma3
            mstore(
                mload(add(vk, 0xa0)),
                18816028553810513067728270242942259651783354986329945635353859047149476279687
            )
            mstore(
                add(mload(add(vk, 0xa0)), 0x20),
                11882680851945303658063593837716037756293837416752296611477056121789431777064
            )
            // sigma4
            mstore(
                mload(add(vk, 0xc0)),
                21510097154791711734296287821852281209791416779989865544015434367940075374914
            )
            mstore(
                add(mload(add(vk, 0xc0)), 0x20),
                3430102751397774877173034066414871678821827985103146314887340992082993984329
            )

            // q1
            mstore(
                mload(add(vk, 0xe0)),
                19869597504326919094166107694290620558808748604476313005465666228287903414344
            )
            mstore(
                add(mload(add(vk, 0xe0)), 0x20),
                7150111322568846997819037419437132637225578315562663408823282538527304893394
            )
            // q2
            mstore(
                mload(add(vk, 0x100)),
                15160992848460929858090744745540508270198264712727437471403260552347088002356
            )
            mstore(
                add(mload(add(vk, 0x100)), 0x20),
                14658479685250391207452586531545916785099257310771621120220342224985727703397
            )
            // q3
            mstore(
                mload(add(vk, 0x120)),
                8235204123369855002620633544318875073465201482729570929826842086900101734240
            )
            mstore(
                add(mload(add(vk, 0x120)), 0x20),
                1315782571791013709741742522230010040948540142932666264718230624795003912658
            )
            // q4
            mstore(
                mload(add(vk, 0x140)),
                7021080634443416008459948952678027962506306501245829421538884411847588184010
            )
            mstore(
                add(mload(add(vk, 0x140)), 0x20),
                6584493294015254847476792897094566004873857428175399671267891995703671301938
            )

            // qM12
            mstore(
                mload(add(vk, 0x160)),
                19199743165408884046745846028664619315169170959180153012829728401858950581623
            )
            mstore(
                add(mload(add(vk, 0x160)), 0x20),
                14838749009602762930836652487207610572239367359059811743491751753845995666312
            )
            // qM34
            mstore(
                mload(add(vk, 0x180)),
                10248259393969855960972127876087560001222739594880062140977367664638629457979
            )
            mstore(
                add(mload(add(vk, 0x180)), 0x20),
                3405469462517204071666729973707416410254082166076974198995581327928518673875
            )

            // qO
            mstore(
                mload(add(vk, 0x1a0)),
                9259807925511910228709408577417518144465439748546649497440413244416264053909
            )
            mstore(
                add(mload(add(vk, 0x1a0)), 0x20),
                4349742126987923639436565898601499377373071260693932114899380098788981806520
            )
            // qC
            mstore(
                mload(add(vk, 0x1c0)),
                195924708408078159303893377539882303047203274957430754688974876101940076523
            )
            mstore(
                add(mload(add(vk, 0x1c0)), 0x20),
                2730242103617344574903225508726280194241425124842703262405260488972083367491
            )
            // qH1
            mstore(
                mload(add(vk, 0x1e0)),
                20219387287202350426068670038890996732790822982376234641416083193417653609683
            )
            mstore(
                add(mload(add(vk, 0x1e0)), 0x20),
                4712902992473903996354956065401616044154872569903741964754702810524685939510
            )
            // qH2
            mstore(
                mload(add(vk, 0x200)),
                20606018511516306199576247848201856706631620007530428100607004704631466340548
            )
            mstore(
                add(mload(add(vk, 0x200)), 0x20),
                3431535724436156106895017518971445784357440465218022981124980111332355382620
            )
            // qH3
            mstore(
                mload(add(vk, 0x220)),
                16926802729258759088538388518776752987858809292908095720269836387951179849328
            )
            mstore(
                add(mload(add(vk, 0x220)), 0x20),
                17982233223518308144739071673627895392237126231063756253762501987899411496611
            )
            // qH4
            mstore(
                mload(add(vk, 0x240)),
                2769108222659962988853179530681878069454558991374977224908414446449310780711
            )
            mstore(
                add(mload(add(vk, 0x240)), 0x20),
                1229799452453481995415811771099188864368739763357472273935665649735041438448
            )
            // qEcc
            mstore(
                mload(add(vk, 0x260)),
                4813470345909172814186147928188285492437945113396806975178500704379725081570
            )
            mstore(
                add(mload(add(vk, 0x260)), 0x20),
                5911983361843136694947821727682990071782684402361679071602671084421707986423
            )
        }
    }
}
