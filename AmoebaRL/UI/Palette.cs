﻿using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using RLNET;
using RogueSharp;

namespace AmoebaRL.UI
{
    /// <summary>
    /// <see cref="https://faronbracy.github.io/RogueSharp/articles/04_color_palette.html"/>
    /// Honestly would prefer to read this from a file but whatever. 7drl means go fast.
    /// </summary>
    public static class Palette
    {
        // http://paletton.com/#uid=73d0u0k5qgb2NnT41jT74c8bJ8X

        public static RLColor PrimaryLightest = new RLColor(110, 121, 119);
        public static RLColor PrimaryLighter = new RLColor(88, 100, 98);
        public static RLColor Primary = new RLColor(68, 82, 79);
        public static RLColor PrimaryDarker = new RLColor(48, 61, 59);
        public static RLColor PrimaryDarkest = new RLColor(29, 45, 42);

        public static RLColor SecondaryLightest = new RLColor(116, 120, 126);
        public static RLColor SecondaryLighter = new RLColor(93, 97, 105);
        public static RLColor Secondary = new RLColor(72, 77, 85);
        public static RLColor SecondaryDarker = new RLColor(51, 56, 64);
        public static RLColor SecondaryDarkest = new RLColor(31, 38, 47);

        public static RLColor AlternateLightest = new RLColor(190, 184, 174);
        public static RLColor AlternateLighter = new RLColor(158, 151, 138);
        public static RLColor Alternate = new RLColor(129, 121, 107);
        public static RLColor AlternateDarker = new RLColor(97, 89, 75);
        public static RLColor AlternateDarkest = new RLColor(71, 62, 45);

        public static RLColor ComplimentLightest = new RLColor(190, 180, 174);
        public static RLColor ComplimentLighter = new RLColor(158, 147, 138);
        public static RLColor Compliment = new RLColor(129, 116, 107);
        public static RLColor ComplimentDarker = new RLColor(97, 84, 75);
        public static RLColor ComplimentDarkest = new RLColor(71, 56, 45);

        // http://pixeljoint.com/forum/forum_posts.asp?TID=12795

        public static RLColor DbDark = new RLColor(20, 12, 28);
        public static RLColor DbOldBlood = new RLColor(68, 36, 52);
        public static RLColor DbDeepWater = new RLColor(48, 52, 109);
        public static RLColor DbOldStone = new RLColor(78, 74, 78);
        public static RLColor DbWood = new RLColor(133, 76, 48);
        public static RLColor DbVegetation = new RLColor(52, 101, 36);
        public static RLColor DbBlood = new RLColor(208, 70, 72);
        public static RLColor DbStone = new RLColor(117, 113, 97);
        public static RLColor DbWater = new RLColor(89, 125, 206);
        public static RLColor DbBrightWood = new RLColor(210, 125, 44);
        public static RLColor DbMetal = new RLColor(133, 149, 161);
        public static RLColor DbGrass = new RLColor(109, 170, 44);
        public static RLColor DbSkin = new RLColor(210, 170, 153);
        public static RLColor DbSky = new RLColor(109, 194, 202);
        public static RLColor DbSun = new RLColor(218, 212, 94);
        public static RLColor DbLight = new RLColor(222, 238, 214);

        // Game Uses
        public static RLColor FloorBackground = RLColor.Black;
        public static RLColor Floor = AlternateDarkest;
        public static RLColor FloorBackgroundFov = DbDark;
        public static RLColor FloorFov = Alternate;

        public static RLColor WallBackground = SecondaryDarkest;
        public static RLColor Wall = Secondary;
        public static RLColor WallBackgroundFov = SecondaryDarker;
        public static RLColor WallFov = SecondaryLighter;

        public static RLColor TextHeading = DbLight;
        public static RLColor TextBody = DbBrightWood;

        public static RLColor SuperBright = DbLight;
        public static RLColor PlayerInactive = DbOldBlood + new RLColor(20,20,20);

        public static RLColor Slime = DbGrass;
        public static RLColor DarkSlime = DbVegetation;

        // Main playermass slime
        // public static RLColor DbGrass = new RLColor(109, 170, 44);
        // public static RLColor DbVegetation = new RLColor(52, 101, 36);

        // Slime on the path
        public static RLColor PathSlime = new RLColor(132, 190, 56);
        public static RLColor BodySlime = new RLColor(PathSlime.r * 0.75f, PathSlime.g * 0.75f, PathSlime.b * 0.75f);


        public static RLColor City = DbMetal;
        public static RLColor Militia = DbBrightWood;
        public static RLColor RestingMilitia = DbWood;
        public static RLColor Calcium = new RLColor(DbWater.r * 1.2f, DbWater.g *0.9f, DbWater.b * 1.2f);
        public static RLColor RestingTank = DbDeepWater;
        public static RLColor Electronics = DbSun;
        public static RLColor ReticleForeground = DbBlood;
        public static RLColor ReticleBackground = DbOldBlood;

        public static RLColor RootOrganelle = DbBlood;
        public static RLColor OrganelleInactive = DbMetal;

        public static RLColor InactiveGravityCore = DbOldBlood;
        public static RLColor InactiveQuantumCore = new RLColor(RLColor.Magenta.r * 0.6f, RLColor.Magenta.g, RLColor.Magenta.b * 0.6f);
        public static RLColor TerrorCoreActive = new RLColor(RLColor.LightGray.r, RLColor.LightGray.g, RLColor.LightGray.b * 1.2f);

        public static RLColor Cursor = RLColor.LightMagenta;
        public static RLColor DarkCursor = RLColor.Magenta;
        public static RLColor SmartCoreInactive = DbOldStone;
        public static RLColor Overfill = DbSky;

        public static RLColor OrganelleConsoleBG = new RLColor(6, 26, 0);
    }
}
