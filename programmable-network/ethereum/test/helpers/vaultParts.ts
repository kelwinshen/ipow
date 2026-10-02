// A vault's two parts, made before it by its factories, for the vault the
// deployer will deploy next: its address follows from the deployer's nonce,
// two transactions on (the two makes).

export async function makeParts(ethers: any, homeFactory: any, receiptsFactory: any, here: number, peer: number, coin: string) {
  const [deployer] = await ethers.getSigners();
  const nonce = await ethers.provider.getTransactionCount(deployer.address);
  const core = ethers.getCreateAddress({ from: deployer.address, nonce: nonce + 2 });
  const home = await homeFactory.connect(deployer).make.staticCall(core, here, peer, coin);
  await (await homeFactory.connect(deployer).make(core, here, peer, coin)).wait();
  const receipts = await receiptsFactory.connect(deployer).make.staticCall(core, here, peer);
  await (await receiptsFactory.connect(deployer).make(core, here, peer)).wait();
  return { core, home: home as string, receipts: receipts as string };
}
