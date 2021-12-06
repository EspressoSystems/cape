import { ethers } from "hardhat";

let leaf_value0 = ethers.BigNumber.from(
  "17101599813294219906421080963940202236242422543188383858545041456174912634952"
);

let zero = ethers.BigNumber.from(0);

let rootValue = ethers.BigNumber.from(
  "16338819200219295738128869281163133642735762710891814031809540606861827401155"
);
let flattenedFrontier0TreeHeight3 = [leaf_value0, zero, zero, zero, zero, zero, zero];

let flattenedFrontier0TreeHeight20 = Array(20 * 2 + 1).fill(0);
flattenedFrontier0TreeHeight20[0] = leaf_value0;

let flattenedFrontier1TreeHeight20 = Array(20 * 2 + 1).fill(zero);

flattenedFrontier1TreeHeight20[0] = ethers.BigNumber.from(
  "19747117328088312725307450364162578532472307437548715261720267275089539145056"
);

flattenedFrontier1TreeHeight20[1] = ethers.BigNumber.from(
  "21675598143565309359812993239757720936435758585734504991083962560787283511262"
);

flattenedFrontier1TreeHeight20[2] = ethers.BigNumber.from(
  "761309565771814004840941655242219578825092544220097759337230034949715648434"
);

flattenedFrontier1TreeHeight20[3] = ethers.BigNumber.from(
  "16524642399970308552658666492759234240026659072930153071186054857997347371974"
);

export {
  rootValue,
  flattenedFrontier0TreeHeight3,
  flattenedFrontier0TreeHeight20,
  flattenedFrontier1TreeHeight20,
};
