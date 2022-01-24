// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "../interfaces/IPlonkVerifier.sol";
import "./BN254.sol";

library Transfer2In2Out24DepthVk {
    function getVk() internal pure returns (IPlonkVerifier.VerifyingKey memory vk) {
        assembly {
            // domain size
            mstore(vk, 32768)
            // num of public inputs
            mstore(add(vk, 0x20), 27)

            // sigma0
            mstore(
                mload(add(vk, 0x40)),
                7628022919529421911135408904372797627127922903613932517951676759551756614275
            )
            mstore(
                add(mload(add(vk, 0x40)), 0x20),
                1331524275175103536317606472081114729669777307477986149584111942393705962450
            )
            // sigma1
            mstore(
                mload(add(vk, 0x60)),
                11385474208217093339197684484172860602491108849062309339809203517524255705814
            )
            mstore(
                add(mload(add(vk, 0x60)), 0x20),
                14742740373953540087108822854363852587371950907295700017218827187367528919422
            )
            // sigma2
            mstore(
                mload(add(vk, 0x80)),
                16656283720893277505520180576834218330228640426319787818259624147689712896181
            )
            mstore(
                add(mload(add(vk, 0x80)), 0x20),
                13325231528863913137181084237184355058186595356556894827411039178877487474770
            )
            // sigma3
            mstore(
                mload(add(vk, 0xa0)),
                9189791310770551336126945048086887553526802063485610994702148384774531567947
            )
            mstore(
                add(mload(add(vk, 0xa0)), 0x20),
                14841018178006034931401800499802155298679474918739530511330632965796343701845
            )
            // sigma4
            mstore(
                mload(add(vk, 0xc0)),
                2291377454368026633206063421914664920045658737580871725587615825936361194543
            )
            mstore(
                add(mload(add(vk, 0xc0)), 0x20),
                1302015066005114004951991020555380375564758415605740891074815812171114380677
            )

            // q1
            mstore(
                mload(add(vk, 0xe0)),
                20820380636256451755441529461019761091650992355545157191471886785846828368458
            )
            mstore(
                add(mload(add(vk, 0xe0)), 0x20),
                21593297517126223340469128837410501412961385490498992377256325174187721359792
            )
            // q2
            mstore(
                mload(add(vk, 0x100)),
                18739722115441254876917366518913137925104098218293815822076739449944538511463
            )
            mstore(
                add(mload(add(vk, 0x100)), 0x20),
                21704728059513369811801942736237462547455258303739352819235283602004201892046
            )
            // q3
            mstore(
                mload(add(vk, 0x120)),
                14641591741781012837232454331455337179912058515648809221995273046957404689696
            )
            mstore(
                add(mload(add(vk, 0x120)), 0x20),
                7809440494808817863276605374028021971161141718007334574770841741782286482045
            )
            // q4
            mstore(
                mload(add(vk, 0x140)),
                12825820090241151628814776520261182308841765265286885643232345964438926321859
            )
            mstore(
                add(mload(add(vk, 0x140)), 0x20),
                953744090209979424715539850359172951613856725623925496688974323728989047678
            )

            // qM12
            mstore(
                mload(add(vk, 0x160)),
                12851524982620297419850126451077057609693331882274130781000694680394484937072
            )
            mstore(
                add(mload(add(vk, 0x160)), 0x20),
                275368615438300238729991830030823846019265755187066004752089508827060302546
            )
            // qM34
            mstore(
                mload(add(vk, 0x180)),
                5220853497691242543339709197361896971155747151782855394304800304146652028430
            )
            mstore(
                add(mload(add(vk, 0x180)), 0x20),
                9450857245879300465114294127329293155426034414913673478235624018652474647192
            )

            // qO
            mstore(
                mload(add(vk, 0x1a0)),
                1021365006885138582377179911145719040433890015638098596677854082251708776428
            )
            mstore(
                add(mload(add(vk, 0x1a0)), 0x20),
                11359935238758701707761945142588661021143398751723216197162452144578378060887
            )
            // qC
            mstore(
                mload(add(vk, 0x1c0)),
                13464643739714429050907960983453767858349630205445421978818631227665532763905
            )
            mstore(
                add(mload(add(vk, 0x1c0)), 0x20),
                10339488547668992208892459748774743478364544079101005770106713704130208623574
            )
            // qH1
            mstore(
                mload(add(vk, 0x1e0)),
                9601738305327057050177966434793538325547418147491497810469219037972470343030
            )
            mstore(
                add(mload(add(vk, 0x1e0)), 0x20),
                19301188629352152421673613863134089760610229764460440766611052882385794236638
            )
            // qH2
            mstore(
                mload(add(vk, 0x200)),
                21079123752659904011291969128982548366933951092885387880640877829556396468124
            )
            mstore(
                add(mload(add(vk, 0x200)), 0x20),
                8511476146119618724794262516873338224284052557219121087531014728412456998247
            )
            // qH3
            mstore(
                mload(add(vk, 0x220)),
                15303909812921746731917671857484723288453878023898728858584106908662401059224
            )
            mstore(
                add(mload(add(vk, 0x220)), 0x20),
                18170356242761746817628282114440738046388581044315241707586116980550978579010
            )
            // qH4
            mstore(
                mload(add(vk, 0x240)),
                4268233897088460316569641617170115742335233153775249443326146549729427293896
            )
            mstore(
                add(mload(add(vk, 0x240)), 0x20),
                18974976451146753275247755359852354432882026367027102555776389253422694257840
            )
            // qEcc
            mstore(
                mload(add(vk, 0x260)),
                14659915475225256091079096704713344128669967309925492152251233149380462089822
            )
            mstore(
                add(mload(add(vk, 0x260)), 0x20),
                2059804379395436696412483294937073085747522899756612651966178273428617505712
            )
        }
    }
}
