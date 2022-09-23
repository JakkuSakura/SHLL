package shll

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test
import shll.ast.AST
import shll.backends.PrettyPrinter
import shll.frontends.ShllLexerAndParser
import shll.optimizers.Specializer

class ShllAstTest {
  val pp: PrettyPrinter = PrettyPrinter()
  val showProgress = true
  def printAst(input: AST): Unit = {
    println(pp.print(input))
  }

  def specializedEquals(input: String, expected: String): Unit = {
    if (showProgress)
        println(s"Parsing $input")
    val ast = ShllLexerAndParser().parse(input)
    if (showProgress)
        println(s"Specializing " + pp.print(ast))
    val optimized = Specializer().specialize(ast)
    if (showProgress)
        println(s"Optimized " + pp.print(optimized))
    val exp = ShllLexerAndParser().parse(expected)
    if (exp != optimized)
      if (showProgress)
          println(s"Expected " + pp.print(exp))
      assertEquals(exp, optimized)
  }
  @Test def testParser(): Unit = {
    val t = ShllLexerAndParser().parse("(block (A) (B))")
    printAst(t)
  }

  @Test def testSpecializer(): Unit = {
    specializedEquals(
      """
        |(block
        |   (def-fun foo (list (field a [int])) [int]
        |     a
        |   )
        |   (foo 1)
        |)
        |""".stripMargin,
      """
        |(block
        |  (def-fun foo_0 (list) [int] a)
        |  (def-fun foo (list (field a [int])) [int] a)
        |  (foo_0)
        |)
        |""".stripMargin
    )
  }

  @Test def testStruct(): Unit = {
    specializedEquals(
      """
        |(block
        |   (def-struct Foo (list (field a [int])))
        |   (select (Foo a=1) a)
        |)
        |""".stripMargin,
      """
        |(block
        |  (def-struct Foo (list (field a [int])))
        |  1
        |)
        |""".stripMargin
    )
  }

}
