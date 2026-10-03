// Runs Context Mapper's Xtext validator on CML files, with every check (CheckMode.ALL): the
// semantic rules too, which the CLI's `cm validate` does not run (it reports the syntax only, and
// exits 0 whatever it finds; DESIGN 0.4 of sakai). One line per issue,
// `<severity> <file>:<line>:<column>: <message>`, and exit 1 when there is an error.
//
// Build: javac -cp "context-mapper-cli-6.12.0/lib/*" -d <dir> Validate.java
// Run:   java -cp "context-mapper-cli-6.12.0/lib/*:<dir>" Validate <file.cml>...
import java.util.List;
import org.contextmapper.dsl.ContextMappingDSLStandaloneSetup;
import org.eclipse.emf.common.util.URI;
import org.eclipse.emf.ecore.resource.Resource;
import org.eclipse.xtext.resource.XtextResource;
import org.eclipse.xtext.resource.XtextResourceSet;
import org.eclipse.xtext.util.CancelIndicator;
import org.eclipse.xtext.validation.CheckMode;
import org.eclipse.xtext.validation.IResourceValidator;
import org.eclipse.xtext.validation.Issue;
import com.google.inject.Injector;

public class Validate {
  public static void main(String[] args) {
    Injector injector = new ContextMappingDSLStandaloneSetup().createInjectorAndDoEMFRegistration();
    XtextResourceSet rs = injector.getInstance(XtextResourceSet.class);
    int errors = 0;
    for (String path : args) {
      Resource r = rs.getResource(URI.createFileURI(new java.io.File(path).getAbsolutePath()), true);
      IResourceValidator v = ((XtextResource) r).getResourceServiceProvider().getResourceValidator();
      List<Issue> issues = v.validate(r, CheckMode.ALL, CancelIndicator.NullImpl);
      for (Issue i : issues) {
        System.out.println(i.getSeverity() + " " + path + ":" + i.getLineNumber() + ":" + i.getColumn() + ": " + i.getMessage());
        if (i.getSeverity() == org.eclipse.xtext.diagnostics.Severity.ERROR) errors++;
      }
    }
    System.exit(errors > 0 ? 1 : 0);
  }
}
