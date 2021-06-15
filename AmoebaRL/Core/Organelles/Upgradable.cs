using AmoebaRL.Interfaces;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Core.Organelles
{
    public abstract class Upgradable : Organelle, IUpgradable
    {
        public delegate Organelle UpgradeResult();

        public class UpgradePath
        {
            public int AmountRequired { get; set; }

            public CraftingMaterial.Resource TypeRequired { get; set; }

            public UpgradeResult Result;

            public UpgradePath(int amountRequired, CraftingMaterial.Resource typeRequired, UpgradeResult result)
            {
                AmountRequired = amountRequired;
                TypeRequired = typeRequired;
                Result = result;
            }

            // ToMessage here.
        }

        public virtual List<UpgradePath> PossiblePaths { get; protected set; } = new List<UpgradePath>();

        public UpgradePath CurrentPath { get; protected set; } = null;

        public int Progress { get; protected set; } = 0;

        public override List<Item> Components()
        {
            List<Item> craftingItems = OrganelleComponents();
            if(CurrentPath != null)
            {
                if (CurrentPath.TypeRequired == CraftingMaterial.Resource.CALCIUM)
                    for (int i = 0; i < CurrentPath.AmountRequired; i++)
                        craftingItems.Add(new CalciumDust());
                else if(CurrentPath.TypeRequired == CraftingMaterial.Resource.ELECTRONICS)
                    for (int i = 0; i < CurrentPath.AmountRequired; i++)
                        craftingItems.Add(new SiliconDust());
            }
            return craftingItems;
        }

        /// <summary>
        /// The components which make up the organelle, ignoring crafting.
        /// </summary>
        /// <returns>The components which make up the organelle, ignoring crafting.</returns>
        public abstract List<Item> OrganelleComponents();


        public bool Upgrade(CraftingMaterial.Resource material)
        {
            if(CurrentPath == null)
            {
                foreach(UpgradePath p in PossiblePaths)
                {
                    if(p.TypeRequired == material)
                    {
                        CurrentPath = p;
                        break;
                    }
                }
            }
            if(CurrentPath != null)
            {
                if(CurrentPath.TypeRequired == material)
                {
                    Progress++;
                    if (Progress >= CurrentPath.AmountRequired)
                    {
                        // Complete the process.
                        string oldName = Name;
                        Map.RemoveActor(this);
                        Actor result = BecomeActor(CurrentPath.Result());
                        Map.PlayerMass.Add(result);
                        Map.UpdatePlayerFieldOfView(); // Otherwise crafted material might not be shown.
                        if (result is Nucleus n) // Edge case; removing/adding scheduled player can cause unwanted swapping.
                            n.SetAsActiveNucleus();
                        Map.Context.MessageLog.Add($"The {oldName} absorbs the {CraftingMaterial.ResourceName(material)} and transforms into a {result.Name}!");
                    }
                    else
                        Map.Context.MessageLog.Add($"The {Name} absorbs the {CraftingMaterial.ResourceName(material)}");
                    return true;
                }
                // Wrong material.
            }
            return false;
        }
    }
}
